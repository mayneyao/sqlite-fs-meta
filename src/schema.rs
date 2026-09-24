use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ColumnDef {
    pub name: String,
    pub data_type: String,
}

#[derive(Debug, Clone)]
pub struct VTabConfig {
    pub root: PathBuf,
    pub namespace: String,
    pub ignore_patterns: Vec<String>,
    pub custom_columns: Vec<ColumnDef>,
    pub delete_physical_file: bool,
}

impl VTabConfig {
    pub fn parse(raw_args: &[String]) -> Result<Self, String> {
        let mut root: Option<PathBuf> = None;
        let mut namespace = "space.eidos.meta".to_string();
        let mut ignore_patterns = vec![
            "node_modules".to_string(),
            "target".to_string(),
            ".git".to_string(),
            ".graft".to_string(),
        ];
        let mut custom_columns = Vec::new();
        let mut delete_physical_file = false;

        for arg in raw_args {
            let trimmed = arg.trim();
            if trimmed.is_empty() {
                continue;
            }

            if let Some((key, val)) = trimmed.split_once('=') {
                let key = key.trim().to_lowercase();
                let val = val.trim().trim_matches('\'').trim_matches('"');

                match key.as_str() {
                    "root" => {
                        root = Some(PathBuf::from(val));
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
                        for col_str in val.split(',') {
                            let col_trimmed = col_str.trim();
                            if col_trimmed.is_empty() {
                                continue;
                            }
                            let parts: Vec<&str> = col_trimmed.split_whitespace().collect();
                            if parts.is_empty() {
                                continue;
                            }
                            let col_name = parts[0].to_string();
                            let col_type = if parts.len() > 1 {
                                parts[1].to_uppercase()
                            } else {
                                "TEXT".to_string()
                            };
                            custom_columns.push(ColumnDef {
                                name: col_name,
                                data_type: col_type,
                            });
                        }
                    }
                    "on_delete" => {
                        if val.eq_ignore_ascii_case("delete_file") {
                            delete_physical_file = true;
                        }
                    }
                    _ => {}
                }
            } else {
                // If it's a positional argument or just root path
                if root.is_none() && !trimmed.contains(' ') {
                    let cleaned = trimmed.trim_matches('\'').trim_matches('"');
                    root = Some(PathBuf::from(cleaned));
                }
            }
        }

        let root = root.ok_or_else(|| "Missing required 'root' argument in fs_meta".to_string())?;

        Ok(VTabConfig {
            root,
            namespace,
            ignore_patterns,
            custom_columns,
            delete_physical_file,
        })
    }

    /// Generates the SQL CREATE TABLE statement required by sqlite3_declare_vtab.
    pub fn to_declare_sql(&self, table_name: &str) -> String {
        let mut cols = vec![
            "\"id\" TEXT PRIMARY KEY".to_string(),
            "\"name\" TEXT".to_string(),
            "\"path\" TEXT".to_string(),
            "\"size\" INTEGER".to_string(),
            "\"mtime\" TEXT".to_string(),
            "\"extension\" TEXT".to_string(),
            "\"is_dir\" INTEGER".to_string(),
        ];

        for col in &self.custom_columns {
            cols.push(format!("\"{}\" {}", col.name, col.data_type));
        }

        format!("CREATE TABLE \"{}\" (\n  {}\n) WITHOUT ROWID;", table_name, cols.join(",\n  "))
    }
}
