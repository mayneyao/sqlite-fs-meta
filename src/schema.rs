use std::path::PathBuf;

fn decode_argument(value: &str) -> Result<String, String> {
    let value = value.trim();
    match value.chars().next() {
        Some(quote @ ('\'' | '"')) => {
            if value.len() < 2 || !value.ends_with(quote) {
                return Err("Unterminated fs_meta argument".into());
            }
            let mut chars = value[1..value.len() - 1].chars().peekable();
            let mut decoded = String::new();
            while let Some(ch) = chars.next() {
                if ch == quote && chars.next() != Some(quote) {
                    return Err("Invalid quote in fs_meta argument".into());
                }
                decoded.push(ch);
            }
            Ok(decoded)
        }
        _ => Ok(value.to_string()),
    }
}

#[derive(Debug, Clone)]
pub struct ColumnDef {
    pub name: String,
    pub data_type: String,
    pub storage_key: String,
}

#[derive(Debug, Clone)]
pub struct VTabConfig {
    pub root: PathBuf,
    pub namespace: String,
    pub ignore_patterns: Vec<String>,
    pub custom_columns: Vec<ColumnDef>,
    pub fields_explicit: bool,
    pub delete_physical_file: bool,
}

impl VTabConfig {
    pub fn parse(raw_args: &[String]) -> Result<Self, String> {
        let mut root: Option<PathBuf> = None;
        let mut namespace = crate::properties::DEFAULT_NAMESPACE.to_string();
        let mut ignore_patterns = vec![
            "node_modules".to_string(),
            "target".to_string(),
            ".git".to_string(),
            ".graft".to_string(),
        ];
        let mut custom_columns = Vec::new();
        let mut fields_explicit = false;
        let mut delete_physical_file = false;

        for arg in raw_args {
            let trimmed = arg.trim();
            if trimmed.is_empty() {
                continue;
            }

            if let Some((key, val)) = trimmed.split_once('=') {
                let key = key.trim().to_lowercase();
                let val = decode_argument(val)?;

                match key.as_str() {
                    "root" => {
                        root = Some(PathBuf::from(&val));
                    }
                    "namespace" => {
                        namespace = val.to_string();
                    }
                    "ignore" => {
                        let additional: Vec<String> = val
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect();
                        ignore_patterns.extend(additional);
                    }
                    "fields" => {
                        fields_explicit = true;
                        if val.trim_start().starts_with('[') {
                            let fields: Vec<crate::folder_schema::Field> =
                                serde_json::from_str(&val)
                                    .map_err(|error| format!("Invalid fields JSON: {error}"))?;
                            for field in fields {
                                let data_type = field.data_type.to_uppercase();
                                if !["TEXT", "INTEGER", "REAL", "BLOB"]
                                    .contains(&data_type.as_str())
                                    || field.name.is_empty()
                                    || field.name.contains('\0')
                                {
                                    return Err("Invalid field definition".into());
                                }
                                custom_columns.push(ColumnDef {
                                    storage_key: field.key.unwrap_or_else(|| field.name.clone()),
                                    name: field.name,
                                    data_type,
                                });
                            }
                            continue;
                        }
                        for col_str in val.split(',') {
                            let col_trimmed = col_str.trim();
                            if col_trimmed.is_empty() {
                                continue;
                            }
                            let parts: Vec<&str> = col_trimmed.split_whitespace().collect();
                            if parts.is_empty() {
                                continue;
                            }
                            if parts.len() > 2 {
                                return Err(format!("Invalid field declaration: {col_trimmed}"));
                            }
                            let col_name = parts[0].to_string();
                            let col_type = if parts.len() > 1 {
                                parts[1].to_uppercase()
                            } else {
                                "TEXT".to_string()
                            };
                            custom_columns.push(ColumnDef {
                                storage_key: col_name.clone(),
                                name: col_name,
                                data_type: col_type,
                            });
                        }
                    }
                    "on_delete" if val.eq_ignore_ascii_case("delete_file") => {
                        delete_physical_file = true;
                    }
                    _ => {}
                }
            } else {
                // If it's a positional argument or just root path
                if root.is_none() {
                    let cleaned = decode_argument(trimmed)?;
                    root = Some(PathBuf::from(cleaned));
                }
            }
        }

        let root = root.ok_or_else(|| "Missing required 'root' argument in fs_meta".to_string())?;
        validate_columns(&custom_columns)?;

        Ok(VTabConfig {
            root,
            namespace,
            ignore_patterns,
            custom_columns,
            fields_explicit,
            delete_physical_file,
        })
    }

    /// Generates the SQL CREATE TABLE statement required by sqlite3_declare_vtab.
    pub fn to_declare_sql(&self, table_name: &str) -> String {
        let mut cols = vec![
            "\"_id\" TEXT PRIMARY KEY".to_string(),
            "\"id\" TEXT".to_string(),
            "\"name\" TEXT".to_string(),
            "\"extension\" TEXT".to_string(),
            "\"size\" INTEGER".to_string(),
            "\"_created_at\" TEXT".to_string(),
            "\"_updated_at\" TEXT".to_string(),
            "\"mtime\" TEXT".to_string(),
            "\"path\" TEXT".to_string(),
            "\"is_dir\" INTEGER".to_string(),
            "\"file\" TEXT".to_string(),
            "\"mimetype\" TEXT".to_string(),
            "\"mime_type\" TEXT".to_string(),
            "\"__fs_meta_remove_key\" TEXT HIDDEN".to_string(),
        ];

        for col in &self.custom_columns {
            cols.push(format!(
                "\"{}\" {}",
                col.name.replace('"', "\"\""),
                col.data_type
            ));
        }

        format!(
            "CREATE TABLE \"{}\" (\n  {}\n) WITHOUT ROWID;",
            table_name.replace('"', "\"\""),
            cols.join(",\n  ")
        )
    }
}

/// Validate declarations once for explicit SQL fields and folder schemas alike.
pub fn validate_columns(columns: &[ColumnDef]) -> Result<(), String> {
    let reserved = [
        "_id",
        "id",
        "name",
        "extension",
        "size",
        "_created_at",
        "_updated_at",
        "mtime",
        "path",
        "is_dir",
        "file",
        "mimetype",
        "mime_type",
        "__fs_meta_remove_key",
    ];
    let mut names = std::collections::HashSet::new();
    for column in columns {
        if column.name.is_empty()
            || column.name.contains('\0')
            || reserved.contains(&column.name.to_ascii_lowercase().as_str())
            || !names.insert(column.name.to_ascii_lowercase())
        {
            return Err(format!(
                "Invalid, reserved, or duplicate field name: {}",
                column.name
            ));
        }
        if !["TEXT", "INTEGER", "REAL", "BLOB"].contains(&column.data_type.as_str()) {
            return Err(format!("Invalid field type: {}", column.data_type));
        }
        crate::properties::validate_key(&column.storage_key).map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_fields_separate_sql_names_from_storage_keys() {
        let config = VTabConfig::parse(&[
            "root='.'".into(),
            r#"fields='[{"name":"Owner''s review, 备注","type":"TEXT","key":"stable-key"}]'"#
                .into(),
        ])
        .unwrap();
        assert_eq!(config.custom_columns[0].name, "Owner's review, 备注");
        assert_eq!(config.custom_columns[0].storage_key, "stable-key");
        assert!(VTabConfig::parse(&[
            "root='.'".into(),
            r#"fields='[{"name":"bad","type":"TEXT); DROP TABLE files"}]'"#.into()
        ])
        .is_err());
    }

    #[test]
    fn decodes_sql_quotes_without_trimming_path_characters() {
        assert_eq!(
            decode_argument("'O''Brien/notes'").unwrap(),
            "O'Brien/notes"
        );
        assert_eq!(decode_argument("'folder\"'").unwrap(), "folder\"");
        assert_eq!(decode_argument("\"a\"\"b\"").unwrap(), "a\"b");
        assert!(decode_argument("'a'b'").is_err());
        assert!(decode_argument("'open").is_err());
    }
}
