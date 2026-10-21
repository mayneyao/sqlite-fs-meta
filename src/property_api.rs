//! SQL adapters for namespace-level property operations.
use serde_json::{Map, Value};
use sqlite_loadable::{api, define_scalar_function, ext::sqlite3, prelude::*, Error, Result};
use std::path::Path;

fn invalid(error: impl ToString) -> Error {
    Error::new_message(error.to_string())
}

pub fn register(db: *mut sqlite3) -> Result<()> {
    define_scalar_function(
        db,
        "fs_meta_read",
        2,
        |context, args| {
            let path = Path::new(api::value_text_notnull(&args[0])?);
            let namespace = api::value_text_notnull(&args[1])?;
            let values = crate::properties::read(path, namespace).map_err(invalid)?;
            api::result_text(context, serde_json::to_string(&values).map_err(invalid)?)
        },
        FunctionFlags::UTF8 | FunctionFlags::DIRECTONLY,
    )?;
    define_scalar_function(
        db,
        "fs_meta_patch",
        4,
        |context, args| {
            let path = Path::new(api::value_text_notnull(&args[0])?);
            let namespace = api::value_text_notnull(&args[1])?;
            crate::properties::validate_namespace(namespace).map_err(invalid)?;
            let set: Map<String, Value> =
                serde_json::from_str(api::value_text_notnull(&args[2])?).map_err(invalid)?;
            let remove: Vec<String> =
                serde_json::from_str(api::value_text_notnull(&args[3])?).map_err(invalid)?;
            let values =
                crate::properties::patch(path, namespace, set, &remove).map_err(invalid)?;
            api::result_text(context, serde_json::to_string(&values).map_err(invalid)?)
        },
        FunctionFlags::UTF8 | FunctionFlags::DIRECTONLY,
    )?;
    Ok(())
}
