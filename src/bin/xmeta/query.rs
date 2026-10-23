//! A small host for the existing extension, not a second filesystem query engine.
use crate::{args::QueryArgs, error::CliError, output};
use fs_meta::folder_schema;
use rusqlite::{
    hooks::{AuthAction, AuthContext, Authorization},
    types::{Value as SqlValue, ValueRef},
    Connection,
};
use serde_json::{json, Map, Value};
use std::ffi::{c_char, c_int, c_void};
use std::time::{Duration, Instant};

type EntryPoint = unsafe extern "C" fn(*mut c_void, *mut *mut c_char, *mut c_void) -> c_int;
unsafe extern "C" {
    fn sqlite3_fsmeta_init(db: *mut c_void, error: *mut *mut c_char, api: *mut c_void) -> c_int;
}

fn sql_error(error: rusqlite::Error) -> CliError {
    CliError::invalid(format!("query failed: {error}"))
}

fn quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}
fn identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

pub fn run(namespace: &str, json_output: bool, args: QueryArgs) -> Result<(), CliError> {
    // sqlite3_auto_extension supplies the same SQLite API table as .load does.
    // The entry point and all filesystem semantics are the shipped extension's.
    unsafe {
        let init: rusqlite::auto_extension::RawAutoExtension =
            std::mem::transmute(sqlite3_fsmeta_init as EntryPoint);
        rusqlite::auto_extension::register_auto_extension(init).map_err(sql_error)?;
    }
    let root =
        std::fs::canonicalize(&args.path).map_err(|error| CliError::input_io(&args.path, error))?;
    // Validate the original spelling too, so canonicalization never hides a link.
    let schema = folder_schema::read(&args.path, namespace)
        .map_err(|error| CliError::from_property(&args.path, namespace, error))?;
    let root = root
        .to_str()
        .ok_or_else(|| CliError::invalid("query root is not UTF-8"))?;
    let db = Connection::open_in_memory().map_err(sql_error)?;
    db.execute_batch(&format!(
        "CREATE VIRTUAL TABLE files USING fs_meta(root={}, namespace={}); PRAGMA query_only=ON;",
        quoted(root),
        quoted(namespace)
    ))
    .map_err(sql_error)?;
    db.authorizer(Some(|context: AuthContext<'_>| match context.action {
        AuthAction::Select | AuthAction::Read { .. } | AuthAction::Recursive => {
            Authorization::Allow
        }
        AuthAction::Function { function_name }
            if !matches!(
                function_name.to_ascii_lowercase().as_str(),
                "fs_meta_patch" | "load_extension"
            ) =>
        {
            Authorization::Allow
        }
        _ => Authorization::Deny,
    }))
    .map_err(sql_error)?;
    let started = Instant::now();
    let timeout = Duration::from_millis(u64::from(args.timeout_ms));
    db.progress_handler(1000, Some(move || started.elapsed() >= timeout))
        .map_err(sql_error)?;
    let sql = args.sql.unwrap_or_else(|| {
        let mut columns = vec!["id".to_string(), "size".to_string()];
        columns.extend(schema.fields.iter().map(|field| identifier(&field.name)));
        format!(
            "SELECT {} FROM files WHERE is_dir = 0 ORDER BY id",
            columns.join(", ")
        )
    });
    let parameters: Vec<Value> = serde_json::from_str(&args.params)
        .map_err(|error| CliError::invalid(format!("--params must be a JSON array: {error}")))?;
    let parameters: Vec<SqlValue> = parameters
        .into_iter()
        .map(bind_value)
        .collect::<Result<_, _>>()?;
    let mut statement = db.prepare(&sql).map_err(sql_error)?;
    if !statement.readonly() || statement.column_count() == 0 {
        return Err(CliError::usage("query accepts one read-only SQL statement"));
    }
    let columns: Vec<String> = statement
        .column_names()
        .iter()
        .map(|name| (*name).to_owned())
        .collect();
    let mut unique = std::collections::HashSet::new();
    if columns.iter().any(|column| !unique.insert(column.clone())) {
        return Err(CliError::usage(
            "query column names must be unique; use AS aliases",
        ));
    }
    let mut cursor = statement
        .query(rusqlite::params_from_iter(parameters))
        .map_err(sql_error)?;
    for _ in 0..args.offset {
        if cursor.next().map_err(sql_error)?.is_none() {
            break;
        }
    }
    let mut rows = Vec::new();
    let mut has_more = false;
    while let Some(row) = cursor.next().map_err(sql_error)? {
        if rows.len() == args.limit as usize {
            has_more = true;
            break;
        }
        let mut values = Map::new();
        for (index, name) in columns.iter().enumerate() {
            let value = match row.get_ref(index).map_err(sql_error)? {
                ValueRef::Null => Value::Null,
                ValueRef::Integer(value) => json!(value),
                ValueRef::Real(value) => json!(value),
                ValueRef::Text(value) => Value::String(
                    String::from_utf8(value.to_vec())
                        .map_err(|_| CliError::invalid("query returned invalid UTF-8 text"))?,
                ),
                ValueRef::Blob(value) => {
                    json!({"$blob": value.iter().map(|byte| format!("{byte:02x}")).collect::<String>()})
                }
            };
            values.insert(name.clone(), value);
        }
        rows.push(Value::Object(values));
    }
    if json_output {
        output::json(
            &json!({ "columns": columns, "rows": rows, "limit": args.limit, "offset": args.offset, "has_more": has_more }),
            true,
        )
    } else {
        let mut lines = vec![columns.join("\t")];
        for row in &rows {
            lines.push(
                columns
                    .iter()
                    .map(|name| cell(&row[name]))
                    .collect::<Vec<_>>()
                    .join("\t"),
            );
        }
        output::text_lines(&lines)?;
        eprintln!(
            "{} row(s){}",
            rows.len(),
            if has_more {
                "; more available with --offset"
            } else {
                ""
            }
        );
        Ok(())
    }
}

fn cell(value: &Value) -> String {
    match value {
        Value::Null => "NULL".into(),
        Value::String(value) => value
            .replace('\\', "\\\\")
            .replace('\t', "\\t")
            .replace('\n', "\\n")
            .replace('\r', "\\r"),
        other => other.to_string(),
    }
}

fn bind_value(value: Value) -> Result<SqlValue, CliError> {
    Ok(match value {
        Value::Null => SqlValue::Null,
        Value::Bool(value) => SqlValue::Integer(i64::from(value)),
        Value::Number(value) if value.is_i64() => SqlValue::Integer(value.as_i64().unwrap()),
        Value::Number(value) if value.is_f64() => SqlValue::Real(value.as_f64().unwrap()),
        Value::Number(_) => {
            return Err(CliError::invalid(
                "integer parameter exceeds SQLite's signed 64-bit range",
            ))
        }
        Value::String(value) => SqlValue::Text(value),
        other => SqlValue::Text(other.to_string()),
    })
}
