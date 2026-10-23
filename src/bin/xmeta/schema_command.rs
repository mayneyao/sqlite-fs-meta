use crate::args::{SchemaArgs, SchemaCommand};
use crate::{commands, error::CliError, output};
use fs_meta::folder_schema::{self, FolderSchema};
use serde_json::Value;

pub fn run(namespace: &str, json: bool, args: SchemaArgs) -> Result<(), CliError> {
    let path = match &args.command {
        SchemaCommand::Get { path }
        | SchemaCommand::Set { path, .. }
        | SchemaCommand::Clear { path } => path,
    };
    let attribute = folder_schema::attribute(namespace)
        .map_err(|error| CliError::from_property(path, namespace, error))?;
    let schema = match &args.command {
        SchemaCommand::Get { path } => folder_schema::read(path, namespace),
        SchemaCommand::Set { path, fields, from } => {
            let schema = if let Some(input) = from {
                serde_json::from_value::<FolderSchema>(Value::Object(commands::read_json_object(
                    input,
                )?))
                .map_err(|error| {
                    CliError::invalid(format!("Invalid folder schema: {error}")).with_path(input)
                })?
            } else {
                FolderSchema::from_fields(fields.as_deref().unwrap_or_default())
                    .map_err(CliError::invalid)?
            };
            folder_schema::write(path, namespace, schema)
        }
        SchemaCommand::Clear { path } => {
            return folder_schema::clear(path, namespace)
                .map_err(|error| CliError::from_property(path, &attribute, error));
        }
    }
    .map_err(|error| CliError::from_property(path, &attribute, error))?;
    output::json(
        &serde_json::to_value(schema).map_err(|error| CliError::invalid(error.to_string()))?,
        json,
    )
}
