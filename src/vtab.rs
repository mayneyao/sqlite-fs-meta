use crate::fs_scanner::{lookup_file, scan_directory, FileInfo};
use crate::meta::{clear_metadata, read_metadata, write_metadata};
use crate::schema::VTabConfig;

use sqlite_loadable::api::{
    result_double, result_int, result_int64, result_null, result_text, value_bytes, value_double,
    value_int64, value_text, value_type, ValueType,
};
use sqlite_loadable::ext::*;
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::slice;

pub const SQLITE_OK: c_int = 0;
pub const SQLITE_ERROR: c_int = 1;

pub const SYSTEM_COLUMNS_COUNT: usize = 7;
pub const COL_ID: usize = 0;
pub const COL_NAME: usize = 1;
pub const COL_PATH: usize = 2;
pub const COL_SIZE: usize = 3;
pub const COL_MTIME: usize = 4;
pub const COL_EXTENSION: usize = 5;
pub const COL_IS_DIR: usize = 6;

#[repr(C)]
pub struct FsMetaVTab {
    pub base: sqlite3_vtab,
    pub config: VTabConfig,
}

#[repr(C)]
pub struct FsMetaCursor {
    pub base: sqlite3_vtab_cursor,
    pub rows: Vec<FileInfo>,
    pub current_idx: usize,
    pub cached_meta: Option<HashMap<String, serde_json::Value>>,
    pub cached_meta_idx: usize,
}

unsafe extern "C" fn vtab_connect(
    db: *mut sqlite3,
    _p_aux: *mut c_void,
    argc: c_int,
    argv: *const *const c_char,
    pp_vtab: *mut *mut sqlite3_vtab,
    pz_err: *mut *mut c_char,
) -> c_int {
    let raw_args = slice::from_raw_parts(argv, argc as usize);
    let mut args = Vec::with_capacity(argc as usize);
    for arg in raw_args {
        if arg.is_null() {
            continue;
        }
        if let Ok(s) = CStr::from_ptr(*arg).to_str() {
            args.push(s.to_string());
        }
    }

    if args.len() < 3 {
        return SQLITE_ERROR as c_int;
    }

    let table_name = &args[2];
    let module_args = if args.len() > 3 { &args[3..] } else { &[] };

    let config = match VTabConfig::parse(module_args) {
        Ok(c) => c,
        Err(err_msg) => {
            let c_err = CString::new(err_msg).unwrap_or_default();
            *pz_err = sqlite3ext_mprintf(c_err.as_ptr());
            return SQLITE_ERROR as c_int;
        }
    };

    let declare_sql = config.to_declare_sql(table_name);
    let c_sql = match CString::new(declare_sql) {
        Ok(s) => s,
        Err(_) => return SQLITE_ERROR as c_int,
    };

    let rc = sqlite3ext_declare_vtab(db, c_sql.as_ptr());
    if rc != SQLITE_OK as c_int {
        return rc;
    }

    let vtab = Box::new(FsMetaVTab {
        base: sqlite3_vtab {
            pModule: std::ptr::null(),
            nRef: 0,
            zErrMsg: std::ptr::null_mut(),
        },
        config,
    });

    *pp_vtab = Box::into_raw(vtab) as *mut sqlite3_vtab;
    SQLITE_OK as c_int
}

unsafe extern "C" fn vtab_disconnect(p_vtab: *mut sqlite3_vtab) -> c_int {
    if !p_vtab.is_null() {
        let _ = Box::from_raw(p_vtab as *mut FsMetaVTab);
    }
    SQLITE_OK as c_int
}

unsafe extern "C" fn vtab_best_index(
    _p_vtab: *mut sqlite3_vtab,
    p_idx_info: *mut sqlite3_index_info,
) -> c_int {
    let info = &mut *p_idx_info;
    let n_constraints = info.nConstraint as usize;
    let constraints = slice::from_raw_parts(info.aConstraint, n_constraints);
    let usages = slice::from_raw_parts_mut(info.aConstraintUsage, n_constraints);

    // Look for equality constraint on id (col 0) or path (col 2)
    let mut point_lookup_idx = None;
    for (i, c) in constraints.iter().enumerate() {
        if c.usable != 0 && (c.iColumn == COL_ID as i32 || c.iColumn == COL_PATH as i32) && c.op == 2 {
            // op 2 is SQLITE_INDEX_CONSTRAINT_EQ
            point_lookup_idx = Some(i);
            break;
        }
    }

    if let Some(i) = point_lookup_idx {
        usages[i].argvIndex = 1;
        usages[i].omit = 1;
        info.idxNum = 1; // 1 represents point lookup
        info.estimatedCost = 1.0;
        info.estimatedRows = 1;
    } else {
        info.idxNum = 0; // 0 represents full scan
        info.estimatedCost = 1000.0;
        info.estimatedRows = 100;
    }

    SQLITE_OK as c_int
}

unsafe extern "C" fn vtab_open(
    _p_vtab: *mut sqlite3_vtab,
    pp_cursor: *mut *mut sqlite3_vtab_cursor,
) -> c_int {
    let cursor = Box::new(FsMetaCursor {
        base: sqlite3_vtab_cursor {
            pVtab: std::ptr::null_mut(),
        },
        rows: Vec::new(),
        current_idx: 0,
        cached_meta: None,
        cached_meta_idx: usize::MAX,
    });
    *pp_cursor = Box::into_raw(cursor) as *mut sqlite3_vtab_cursor;
    SQLITE_OK as c_int
}

unsafe extern "C" fn vtab_close(p_cursor: *mut sqlite3_vtab_cursor) -> c_int {
    if !p_cursor.is_null() {
        let _ = Box::from_raw(p_cursor as *mut FsMetaCursor);
    }
    SQLITE_OK as c_int
}

unsafe extern "C" fn vtab_filter(
    p_cursor: *mut sqlite3_vtab_cursor,
    idx_num: c_int,
    _idx_str: *const c_char,
    argc: c_int,
    argv: *mut *mut sqlite3_value,
) -> c_int {
    let cursor = &mut *(p_cursor as *mut FsMetaCursor);
    let vtab = &mut *((*p_cursor).pVtab as *mut FsMetaVTab);

    cursor.rows.clear();
    cursor.current_idx = 0;
    cursor.cached_meta = None;
    cursor.cached_meta_idx = usize::MAX;

    if idx_num == 1 && argc > 0 {
        // Point lookup
        let arg0 = *argv;
        if let Ok(id_str) = value_text(&arg0) {
            if let Some(file_info) = lookup_file(&vtab.config.root, id_str) {
                cursor.rows.push(file_info);
            }
        }
    } else {
        // Full directory scan
        cursor.rows = scan_directory(&vtab.config.root, &vtab.config.ignore_patterns);
    }

    SQLITE_OK as c_int
}

unsafe extern "C" fn vtab_next(p_cursor: *mut sqlite3_vtab_cursor) -> c_int {
    let cursor = &mut *(p_cursor as *mut FsMetaCursor);
    cursor.current_idx += 1;
    SQLITE_OK as c_int
}

unsafe extern "C" fn vtab_eof(p_cursor: *mut sqlite3_vtab_cursor) -> c_int {
    let cursor = &*(p_cursor as *mut FsMetaCursor);
    if cursor.current_idx >= cursor.rows.len() {
        1
    } else {
        0
    }
}

unsafe extern "C" fn vtab_column(
    p_cursor: *mut sqlite3_vtab_cursor,
    ctx: *mut sqlite3_context,
    i_col: c_int,
) -> c_int {
    let cursor = &mut *(p_cursor as *mut FsMetaCursor);
    let vtab = &*(cursor.base.pVtab as *mut FsMetaVTab);

    if cursor.current_idx >= cursor.rows.len() {
        result_null(ctx);
        return SQLITE_OK as c_int;
    }

    let file = &cursor.rows[cursor.current_idx];
    let col = i_col as usize;

    match col {
        COL_ID => {
            let _ = result_text(ctx, &file.rel_path);
        }
        COL_NAME => {
            let _ = result_text(ctx, &file.filename);
        }
        COL_PATH => {
            let _ = result_text(ctx, &file.rel_path);
        }
        COL_SIZE => {
            result_int64(ctx, file.size as i64);
        }
        COL_MTIME => {
            let _ = result_text(ctx, &file.mtime_iso);
        }
        COL_EXTENSION => {
            let _ = result_text(ctx, &file.extension);
        }
        COL_IS_DIR => {
            result_int(ctx, if file.is_dir { 1 } else { 0 });
        }
        _ => {
            // User-defined custom metadata column
            let custom_idx = col - SYSTEM_COLUMNS_COUNT;
            if custom_idx < vtab.config.custom_columns.len() {
                let col_name = &vtab.config.custom_columns[custom_idx].name;

                // Lazily load metadata for the current file if not cached
                if cursor.cached_meta_idx != cursor.current_idx {
                    let full_path = vtab.config.root.join(&file.rel_path);
                    cursor.cached_meta = read_metadata(&full_path, &vtab.config.namespace).ok();
                    cursor.cached_meta_idx = cursor.current_idx;
                }

                if let Some(ref meta) = cursor.cached_meta {
                    if let Some(val) = meta.get(col_name) {
                        match val {
                            serde_json::Value::Null => result_null(ctx),
                            serde_json::Value::Bool(b) => result_int(ctx, if *b { 1 } else { 0 }),
                            serde_json::Value::Number(n) => {
                                if let Some(i) = n.as_i64() {
                                    result_int64(ctx, i);
                                } else if let Some(f) = n.as_f64() {
                                    result_double(ctx, f);
                                } else {
                                    result_null(ctx);
                                }
                            }
                            serde_json::Value::String(s) => {
                                let _ = result_text(ctx, s);
                            }
                            serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
                                let serialized = serde_json::to_string(val).unwrap_or_default();
                                let _ = result_text(ctx, &serialized);
                            }
                        }
                        return SQLITE_OK as c_int;
                    }
                }
            }
            result_null(ctx);
        }
    }

    SQLITE_OK as c_int
}

unsafe extern "C" fn vtab_rowid(
    p_cursor: *mut sqlite3_vtab_cursor,
    p_rowid: *mut i64,
) -> c_int {
    let cursor = &*(p_cursor as *mut FsMetaCursor);
    *p_rowid = (cursor.current_idx + 1) as i64;
    SQLITE_OK as c_int
}

unsafe extern "C" fn vtab_update(
    p_vtab: *mut sqlite3_vtab,
    argc: c_int,
    argv: *mut *mut sqlite3_value,
    _p_rowid: *mut i64,
) -> c_int {
    let vtab = &*(p_vtab as *mut FsMetaVTab);
    let args = slice::from_raw_parts(argv, argc as usize);

    if argc == 1 {
        // DELETE operation
        // argv[0] contains the primary key / rowid to delete
        let val0 = args[0];
        let rel_path = match value_text(&val0) {
            Ok(s) => s,
            Err(_) => return SQLITE_ERROR as c_int,
        };
        let full_path = vtab.config.root.join(rel_path);

        if vtab.config.delete_physical_file {
            let _ = std::fs::remove_file(full_path);
        } else {
            let _ = clear_metadata(&full_path, &vtab.config.namespace);
        }
        return SQLITE_OK as c_int;
    }

    if argc > 1 {
        let val0 = args[0];
        // If val0 is NULL, this is an INSERT
        if value_type(&val0) == ValueType::Null {
            // Virtual table does not create physical files on INSERT by default
            return SQLITE_ERROR as c_int;
        }

        // UPDATE operation
        let rel_path = if args.len() > 2 && value_type(&args[2]) == ValueType::Text {
            match value_text(&args[2]) {
                Ok(s) => s,
                Err(_) => return SQLITE_ERROR as c_int,
            }
        } else {
            match value_text(&val0) {
                Ok(s) => s,
                Err(_) => return SQLITE_ERROR as c_int,
            }
        };
        let full_path = vtab.config.root.join(rel_path);

        if !full_path.exists() {
            return SQLITE_ERROR as c_int;
        }

        let mut meta = read_metadata(&full_path, &vtab.config.namespace).unwrap_or_default();

        // Custom columns start at column index 7, which corresponds to args[2 + 7] = args[9]
        for (i, col_def) in vtab.config.custom_columns.iter().enumerate() {
            let arg_idx = 2 + SYSTEM_COLUMNS_COUNT + i;
            if arg_idx < args.len() {
                let col_val = args[arg_idx];
                match value_type(&col_val) {
                    ValueType::Null => {
                        meta.remove(&col_def.name);
                    }
                    ValueType::Integer => {
                        let num = value_int64(&col_val);
                        meta.insert(col_def.name.clone(), serde_json::json!(num));
                    }
                    ValueType::Float => {
                        let f = value_double(&col_val);
                        meta.insert(col_def.name.clone(), serde_json::json!(f));
                    }
                    ValueType::Text => {
                        if let Ok(text) = value_text(&col_val) {
                            if text.is_empty() {
                                meta.remove(&col_def.name);
                            } else {
                                // Try parsing as JSON array/object, otherwise store as string
                                let parsed = match serde_json::from_str::<serde_json::Value>(text) {
                                    Ok(json_obj) => json_obj,
                                    Err(_) => serde_json::Value::String(text.to_string()),
                                };
                                meta.insert(col_def.name.clone(), parsed);
                            }
                        }
                    }
                    ValueType::Blob => {
                        let bytes = value_bytes(&col_val);
                        let _ = bytes;
                    }
                }
            }
        }

        if write_metadata(&full_path, &vtab.config.namespace, &meta).is_err() {
            return SQLITE_ERROR as c_int;
        }

        return SQLITE_OK as c_int;
    }

    SQLITE_ERROR as c_int
}

static FS_META_MODULE: sqlite3_module = sqlite3_module {
    iVersion: 0,
    xCreate: Some(vtab_connect),
    xConnect: Some(vtab_connect),
    xBestIndex: Some(vtab_best_index),
    xDisconnect: Some(vtab_disconnect),
    xDestroy: Some(vtab_disconnect),
    xOpen: Some(vtab_open),
    xClose: Some(vtab_close),
    xFilter: Some(vtab_filter),
    xNext: Some(vtab_next),
    xEof: Some(vtab_eof),
    xColumn: Some(vtab_column),
    xRowid: Some(vtab_rowid),
    xUpdate: Some(vtab_update),
    xBegin: None,
    xSync: None,
    xCommit: None,
    xRollback: None,
    xFindFunction: None,
    xRename: None,
    xSavepoint: None,
    xRelease: None,
    xRollbackTo: None,
    xShadowName: None,
};

pub unsafe fn register_fs_meta_module(db: *mut sqlite3) -> c_int {
    let mod_name = CString::new("fs_meta").unwrap_or_default();
    sqlite3ext_create_module_v2(
        db,
        mod_name.as_ptr(),
        &FS_META_MODULE,
        std::ptr::null_mut(),
        None,
    )
}
