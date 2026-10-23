//! Folder-local field declarations shared by the extension and xmeta.
//! Each metadata namespace has a separate `<namespace>.schema` attribute.
use crate::properties::{self, PropertyError};
use crate::schema::{ColumnDef, VTabConfig};
use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Field {
    pub name: String,
    #[serde(rename = "type")]
    pub data_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FolderSchema {
    pub version: u32,
    pub fields: Vec<Field>,
}

impl Default for FolderSchema {
    fn default() -> Self {
        Self {
            version: 1,
            fields: Vec::new(),
        }
    }
}

impl FolderSchema {
    pub fn columns(&self) -> Result<Vec<ColumnDef>, String> {
        if self.version != 1 {
            return Err(format!(
                "Unsupported folder schema version {}",
                self.version
            ));
        }
        let columns: Vec<_> = self
            .fields
            .iter()
            .map(|field| ColumnDef {
                name: field.name.clone(),
                data_type: field.data_type.to_uppercase(),
                storage_key: field.key.clone().unwrap_or_else(|| field.name.clone()),
            })
            .collect();
        crate::schema::validate_columns(&columns)?;
        Ok(columns)
    }

    pub fn from_fields(fields: &str) -> Result<Self, String> {
        let config = VTabConfig::parse(&[
            "root='.'".into(),
            format!("fields='{}'", fields.replace('\'', "''")),
        ])?;
        Ok(Self {
            version: 1,
            fields: config
                .custom_columns
                .into_iter()
                .map(|column| Field {
                    key: (column.storage_key != column.name).then_some(column.storage_key),
                    name: column.name,
                    data_type: column.data_type,
                })
                .collect(),
        })
    }
}

pub fn attribute(namespace: &str) -> Result<String, PropertyError> {
    properties::validate_namespace(namespace)?;
    let attribute = format!("{namespace}.schema");
    properties::validate_namespace(&attribute)?;
    Ok(attribute)
}

fn invalid(message: impl Into<String>) -> PropertyError {
    io::Error::new(io::ErrorKind::InvalidData, message.into()).into()
}

fn directory(path: &Path) -> Result<(), PropertyError> {
    if !crate::meta::open_metadata_target(path)?
        .metadata()?
        .is_dir()
    {
        return Err(invalid("Folder schema requires a directory"));
    }
    Ok(())
}

pub fn read(path: &Path, namespace: &str) -> Result<FolderSchema, PropertyError> {
    directory(path)?;
    let Some(bytes) = properties::read_raw(path, &attribute(namespace)?)? else {
        return Ok(FolderSchema::default());
    };
    if bytes.len() > properties::MAX_ENVELOPE_BYTES {
        return Err(PropertyError::TooLarge);
    }
    let schema: FolderSchema = serde_json::from_slice(&bytes).map_err(PropertyError::Corrupt)?;
    schema.columns().map_err(invalid)?;
    Ok(schema)
}

pub fn write(
    path: &Path,
    namespace: &str,
    mut schema: FolderSchema,
) -> Result<FolderSchema, PropertyError> {
    directory(path)?;
    schema.columns().map_err(invalid)?;
    for field in &mut schema.fields {
        field.data_type = field.data_type.to_uppercase();
    }
    let value = serde_json::to_value(&schema).map_err(PropertyError::Encode)?;
    properties::replace(
        path,
        &attribute(namespace)?,
        value.as_object().unwrap().clone(),
    )?;
    Ok(schema)
}

pub fn clear(path: &Path, namespace: &str) -> Result<(), PropertyError> {
    directory(path)?;
    properties::clear(path, &attribute(namespace)?)
}
