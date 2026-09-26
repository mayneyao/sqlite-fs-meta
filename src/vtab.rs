use crate::fs_scanner::{lookup_file, scan_directory, FileInfo};
use crate::meta::{clear_metadata, read_metadata, write_metadata};
use crate::schema::VTabConfig;

use sha2::{Digest, Sha256};
use sqlite_loadable::api::{
    result_double, result_int, result_int64, result_null, result_text, value_bytes, value_double,
    value_int64, value_text, value_type, ValueType,
};
use sqlite_loadable::ext::*;
use std::collections::{BTreeMap, HashMap};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::slice;

pub const SQLITE_OK: c_int = 0;
pub const SQLITE_ERROR: c_int = 1;

pub const SYSTEM_COLUMNS_COUNT: usize = 13;
pub const COL__ID: usize = 0;
pub const COL_ID: usize = 1;
pub const COL_NAME: usize = 2;
pub const COL_EXTENSION: usize = 3;
pub const COL_SIZE: usize = 4;
pub const COL__CREATED_AT: usize = 5;
pub const COL__UPDATED_AT: usize = 6;
pub const COL_MTIME: usize = 7;
pub const COL_PATH: usize = 8;
pub const COL_IS_DIR: usize = 9;
pub const COL_FILE: usize = 10;
pub const COL_MIMETYPE: usize = 11;
pub const COL_MIME_TYPE: usize = 12;

pub fn deterministic_uuid_v7(path: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(path.as_bytes());
    let result = hasher.finalize();

    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&result[0..16]);

    // Set version to 7 (0b0111_xxxx in high nibble of byte 6)
    bytes[6] = (bytes[6] & 0x0f) | 0x70;
    // Set variant to RFC 4122 (0b10xx_xxxx in byte 8)
    bytes[8] = (bytes[8] & 0x3f) | 0x80;

    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3],
        bytes[4], bytes[5],
        bytes[6], bytes[7],
        bytes[8], bytes[9],
        bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
    )
}

pub fn guess_media_type(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "avif" => "image/avif",
        "ico" => "image/x-icon",
        "svg" => "image/svg+xml",
        "tiff" | "tif" => "image/tiff",
        "heic" => "image/heic",
        "heif" => "image/heif",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "tar" => "application/x-tar",
        "gz" | "gzip" => "application/gzip",
        "7z" => "application/x-7z-compressed",
        "rar" => "application/vnd.rar",
        "csv" => "text/csv",
        "tsv" => "text/tab-separated-values",
        "json" => "application/json",
        "xml" => "application/xml",
        "yaml" | "yml" => "text/yaml",
        "md" | "markdown" => "text/markdown",
        "txt" => "text/plain",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" | "mjs" | "cjs" => "text/javascript",
        "ts" | "mts" | "cts" => "text/typescript",
        "tsx" | "jsx" => "text/javascript",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "ppt" => "application/vnd.ms-powerpoint",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "flac" => "audio/flac",
        "m4a" => "audio/mp4",
        "aac" => "audio/aac",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "mkv" => "video/x-matroska",
        "avi" => "video/x-msvideo",
        _ => "application/octet-stream",
    }
}

pub fn encode_uri_path(path: &str) -> String {
    let mut encoded = String::with_capacity(path.len());
    for b in path.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                encoded.push(b as char);
            }
            _ => {
                use std::fmt::Write;
                let _ = write!(encoded, "%{:02X}", b);
            }
        }
    }
    encoded
}

pub fn format_file_entry(rel_path: &str, filename: &str, ext: &str, size: u64) -> String {
    let id = deterministic_uuid_v7(rel_path);
    let media_type = guess_media_type(ext);
    let size_str = size.to_string();
    let uri = encode_uri_path(rel_path);

    let mut entry = BTreeMap::new();
    entry.insert("id", id.as_str());
    entry.insert("mediaType", media_type);
    entry.insert("name", filename);
    entry.insert("size", size_str.as_str());
    entry.insert("uri", uri.as_str());

    serde_json::to_string(&vec![entry]).unwrap_or_else(|_| "[]".to_string())
}

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

    // Look for equality constraint on _id (col 0), id (col 1), or path (col 8)
    let mut point_lookup_idx = None;
    for (i, c) in constraints.iter().enumerate() {
        if c.usable != 0
            && (c.iColumn == COL__ID as i32
                || c.iColumn == COL_ID as i32
                || c.iColumn == COL_PATH as i32)
            && c.op == 2
        {
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
        COL__ID | COL_ID | COL_PATH => {
            let _ = result_text(ctx, &file.rel_path);
        }
        COL_NAME => {
            let _ = result_text(ctx, &file.filename);
        }
        COL_EXTENSION => {
            let _ = result_text(ctx, &file.extension);
        }
        COL_SIZE => {
            result_int64(ctx, file.size as i64);
        }
        COL__CREATED_AT | COL__UPDATED_AT | COL_MTIME => {
            let _ = result_text(ctx, &file.mtime_iso);
        }
        COL_IS_DIR => {
            result_int(ctx, if file.is_dir { 1 } else { 0 });
        }
        COL_FILE => {
            if file.is_dir {
                result_null(ctx);
            } else {
                let entry =
                    format_file_entry(&file.rel_path, &file.filename, &file.extension, file.size);
                let _ = result_text(ctx, &entry);
            }
        }
        COL_MIMETYPE | COL_MIME_TYPE => {
            if file.is_dir {
                result_null(ctx);
            } else {
                let media_type = guess_media_type(&file.extension);
                let _ = result_text(ctx, media_type);
            }
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

unsafe extern "C" fn vtab_rowid(p_cursor: *mut sqlite3_vtab_cursor, p_rowid: *mut i64) -> c_int {
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

/// Registers the fs_meta module with SQLite.
///
/// # Safety
///
/// `db` must be a valid, open SQLite database connection pointer.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uuid_v7_format() {
        let uuid = deterministic_uuid_v7("sub/test-image.png");
        assert_eq!(uuid.len(), 36);
        let chars: Vec<char> = uuid.chars().collect();
        assert_eq!(chars[8], '-');
        assert_eq!(chars[13], '-');
        assert_eq!(chars[14], '7'); // version 7
        assert_eq!(chars[18], '-');
        assert!(matches!(chars[19], '8' | '9' | 'a' | 'b')); // variant RFC 4122
        assert_eq!(chars[23], '-');

        // Deterministic check
        let uuid2 = deterministic_uuid_v7("sub/test-image.png");
        assert_eq!(uuid, uuid2);

        // Different path yields different UUID
        let uuid3 = deterministic_uuid_v7("sub/other.png");
        assert_ne!(uuid, uuid3);
    }

    #[test]
    fn test_format_file_entry_canonical_json() {
        let json_str = format_file_entry("photos/cat.png", "cat.png", "png", 1024);
        assert!(json_str.starts_with(r#"[{"id":""#));
        assert!(json_str.contains(r#""mediaType":"image/png""#));
        assert!(json_str.contains(r#""name":"cat.png""#));
        assert!(json_str.contains(r#""size":"1024""#));
        assert!(json_str.contains(r#""uri":"photos/cat.png""#));
        assert!(json_str.ends_with("}]"));

        // Ensure keys are in exact alphabetical order: id, mediaType, name, size, uri
        let id_idx = json_str.find(r#""id""#).unwrap();
        let mt_idx = json_str.find(r#""mediaType""#).unwrap();
        let name_idx = json_str.find(r#""name""#).unwrap();
        let size_idx = json_str.find(r#""size""#).unwrap();
        let uri_idx = json_str.find(r#""uri""#).unwrap();

        assert!(id_idx < mt_idx);
        assert!(mt_idx < name_idx);
        assert!(name_idx < size_idx);
        assert!(size_idx < uri_idx);
    }

    #[test]
    fn test_encode_uri_path_unicode_and_spaces() {
        let uri = encode_uri_path("photos/my photo 2026.png");
        assert_eq!(uri, "photos/my%20photo%202026.png");

        let uri_unicode = encode_uri_path("文档/封面.png");
        assert!(uri_unicode.starts_with("%"));
        assert!(uri_unicode.contains("/"));
        assert!(uri_unicode.ends_with(".png"));
    }
}
