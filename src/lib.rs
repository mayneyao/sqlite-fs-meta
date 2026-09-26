pub mod fs_scanner;
pub mod meta;
pub mod schema;
pub mod vtab;

use crate::vtab::SQLITE_OK;
use sqlite_loadable::ext::sqlite3;
use sqlite_loadable::prelude::*;
use sqlite_loadable::{api, define_scalar_function, Error, Result};

fn init_fs_meta(db: *mut sqlite3) -> Result<()> {
    define_scalar_function(
        db,
        "fs_meta_root_mode",
        0,
        |context, _| api::result_text(context, "database"),
        FunctionFlags::UTF8 | FunctionFlags::DETERMINISTIC,
    )?;
    unsafe {
        let rc = vtab::register_fs_meta_module(db);
        if rc != SQLITE_OK {
            return Err(Error::new_message(
                "Failed to register fs_meta virtual table",
            ));
        }
    }
    Ok(())
}

/// Primary entrypoint: sqlite3_fsmeta_init (standard for libfs_meta or fs_meta)
#[sqlite_entrypoint]
pub fn sqlite3_fsmeta_init(db: *mut sqlite3) -> Result<()> {
    init_fs_meta(db)
}

/// Entrypoint for lib_fs_meta
#[sqlite_entrypoint]
pub fn sqlite3_lib_fs_meta_init(db: *mut sqlite3) -> Result<()> {
    init_fs_meta(db)
}

/// Entrypoint for libfsmeta
#[sqlite_entrypoint]
pub fn sqlite3_libfsmeta_init(db: *mut sqlite3) -> Result<()> {
    init_fs_meta(db)
}

/// Fallback entrypoint matching the library name libsqlite_fs_meta (no underscores)
#[sqlite_entrypoint]
pub fn sqlite3_sqlitefsmeta_init(db: *mut sqlite3) -> Result<()> {
    init_fs_meta(db)
}

/// Fallback entrypoint matching the library name libsqlite_fs_meta (with underscores)
#[sqlite_entrypoint]
pub fn sqlite3_sqlite_fs_meta_init(db: *mut sqlite3) -> Result<()> {
    init_fs_meta(db)
}
