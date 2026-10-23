use crate::args::{
    ClearArgs, Cli, Command, ConfigArgs, ConfigCommand, GetArgs, PatchArgs, SetArgs, UnsetArgs,
};
use crate::config;
use crate::error::{CliError, CliErrorKind};
use crate::output;
use fs_meta::properties::{self, MAX_ENVELOPE_BYTES};
use serde_json::{Map, Value};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

pub fn run(cli: Cli) -> Result<(), CliError> {
    let json = cli.json;
    run_inner(cli).map_err(|error| error.with_json(json))
}

fn run_inner(cli: Cli) -> Result<(), CliError> {
    let Cli {
        namespace,
        config: config_file,
        json,
        command,
    } = cli;

    match command {
        Command::Config(args) => {
            config_command(namespace.as_deref(), config_file.as_deref(), json, args)
        }
        command => {
            let target = command_target(&command);
            let resolved = config::resolve(namespace.as_deref(), config_file.as_deref(), target)?;
            let namespace = resolved.namespace;
            match command {
                Command::Get(args) => get(&namespace, json, args),
                Command::Set(args) => set(&namespace, json, args),
                Command::Unset(args) => unset(&namespace, json, args),
                Command::Patch(args) => patch(&namespace, json, args),
                Command::Clear(args) => clear(&namespace, args),
                Command::Schema(args) => crate::schema_command::run(&namespace, json, args),
                Command::Query(args) => crate::query::run(&namespace, json, args),
                Command::Config(_) => unreachable!(),
            }
        }
    }
}

fn command_target(command: &Command) -> Option<&Path> {
    match command {
        Command::Get(args) => Some(&args.path),
        Command::Set(args) => Some(&args.path),
        Command::Unset(args) => Some(&args.path),
        Command::Patch(args) => Some(&args.path),
        Command::Clear(args) => Some(&args.path),
        Command::Query(args) => Some(&args.path),
        Command::Schema(args) => Some(match &args.command {
            crate::args::SchemaCommand::Get { path }
            | crate::args::SchemaCommand::Set { path, .. }
            | crate::args::SchemaCommand::Clear { path } => path,
        }),
        Command::Config(_) => None,
    }
}

fn config_command(
    cli_namespace: Option<&str>,
    explicit_config: Option<&Path>,
    json_output: bool,
    args: ConfigArgs,
) -> Result<(), CliError> {
    match args.command {
        None => {
            let resolved = config::resolve(cli_namespace, explicit_config, None)?;
            if json_output {
                output::json(&resolved.report(), true)
            } else {
                print_config(&resolved)
            }
        }
        Some(ConfigCommand::Get) => {
            let resolved = config::resolve(cli_namespace, explicit_config, None)?;
            if json_output {
                output::json(&resolved.report(), true)
            } else {
                output::text_lines(&[resolved.namespace])
            }
        }
        Some(ConfigCommand::Set { namespace, project }) => {
            let path = config_target(explicit_config, project)?;
            config::set_namespace(&path, &namespace)
        }
        Some(ConfigCommand::Unset { project }) => {
            let path = config_target(explicit_config, project)?;
            config::unset_namespace(&path)
        }
        Some(ConfigCommand::Path { project }) => {
            let path = config_target(explicit_config, project)?;
            if json_output {
                output::json(&serde_json::json!({ "path": path.to_string_lossy() }), true)
            } else {
                output::text_lines(&[path.display().to_string()])
            }
        }
    }
}

fn config_target(explicit_config: Option<&Path>, project: bool) -> Result<PathBuf, CliError> {
    if project && explicit_config.is_some() {
        return Err(CliError::usage(
            "--config cannot be combined with --project",
        ));
    }
    if let Some(path) = explicit_config {
        return Ok(path.to_path_buf());
    }
    if project {
        config::project_config_target()
    } else {
        config::user_config_target()
    }
}

fn print_config(resolved: &config::Resolved) -> Result<(), CliError> {
    let mut lines = vec![format!("namespace: {}", resolved.namespace)];
    match resolved.source.path() {
        Some(path) => lines.push(format!(
            "source: {} ({})",
            resolved.source.kind(),
            path.display()
        )),
        None => lines.push(format!("source: {}", resolved.source.kind())),
    }
    lines.push(format!(
        "user config: {}",
        resolved.user_path.as_ref().map_or_else(
            || "(unavailable)".to_owned(),
            |path| path.display().to_string()
        )
    ));
    lines.push(format!(
        "project config: {}",
        resolved
            .project_path
            .as_ref()
            .map_or_else(|| "(none)".to_owned(), |path| path.display().to_string())
    ));
    if let Some(path) = &resolved.explicit_path {
        lines.push(format!("explicit config: {}", path.display()));
    }
    output::text_lines(&lines)
}

fn get(namespace: &str, json_output: bool, args: GetArgs) -> Result<(), CliError> {
    if args.raw {
        if !args.keys.is_empty() {
            return Err(CliError::usage("--raw cannot be combined with KEY"));
        }
        if args.text {
            return Err(CliError::usage("--raw cannot be combined with --text"));
        }
        let bytes = properties::read_raw(&args.path, namespace)
            .map_err(|error| CliError::from_property(&args.path, namespace, error))?;
        let Some(bytes) = bytes else {
            return Err(CliError::not_found("property namespace is not set")
                .with_path(&args.path)
                .with_namespace(namespace));
        };
        return output::raw(&bytes);
    }

    if args.text && args.keys.is_empty() {
        return Err(CliError::usage("--text requires at least one KEY"));
    }
    if args.text && json_output {
        return Err(CliError::usage("--text cannot be combined with --json"));
    }

    let values = properties::read(&args.path, namespace)
        .map_err(|error| CliError::from_property(&args.path, namespace, error))?;

    if args.keys.is_empty() {
        return output::json(&Value::Object(values), json_output);
    }

    let mut selected = Vec::with_capacity(args.keys.len());
    for key in &args.keys {
        let Some(value) = values.get(key) else {
            return Err(
                CliError::not_found(format!("property key '{key}' not found"))
                    .with_path(&args.path)
                    .with_namespace(namespace)
                    .with_key(key),
            );
        };
        selected.push((key, value));
    }

    if args.text {
        let mut lines = Vec::with_capacity(selected.len());
        for (key, value) in &selected {
            let Some(text) = scalar_text(value) else {
                return Err(
                    CliError::invalid(format!("property '{key}' is not a scalar"))
                        .with_path(&args.path)
                        .with_namespace(namespace)
                        .with_key(key),
                );
            };
            lines.push(text);
        }
        return output::text_lines(&lines);
    }

    if selected.len() == 1 {
        return output::json(selected[0].1, json_output);
    }

    let mut result = Map::new();
    for (key, value) in selected {
        result.insert(key.clone(), value.clone());
    }
    output::json(&Value::Object(result), json_output)
}

fn set(namespace: &str, json_output: bool, args: SetArgs) -> Result<(), CliError> {
    let mut values = Map::new();
    for raw in &args.assignments {
        let assignment = parse_assignment(raw, args.json_values)?;
        values.insert(assignment.key, assignment.value);
    }

    let result = if args.replace {
        if args.dry_run {
            properties::preview_replace(&args.path, namespace, values)
        } else {
            properties::replace(&args.path, namespace, values)
        }
    } else if args.dry_run {
        properties::preview_patch(&args.path, namespace, values, &[])
    } else {
        properties::patch(&args.path, namespace, values, &[])
    };
    let result = result.map_err(|error| CliError::from_property(&args.path, namespace, error))?;

    if args.dry_run || args.print {
        output::json(&Value::Object(result), json_output)?;
    }
    Ok(())
}

fn unset(namespace: &str, json_output: bool, args: UnsetArgs) -> Result<(), CliError> {
    for key in &args.keys {
        check_key(key)?;
    }

    if args.strict {
        let current = properties::read(&args.path, namespace)
            .map_err(|error| CliError::from_property(&args.path, namespace, error))?;
        for key in &args.keys {
            if !current.contains_key(key) {
                return Err(
                    CliError::not_found(format!("property key '{key}' not found"))
                        .with_path(&args.path)
                        .with_namespace(namespace)
                        .with_key(key),
                );
            }
        }
    }

    let result = if args.dry_run {
        properties::preview_patch(&args.path, namespace, Map::new(), &args.keys)
    } else {
        properties::patch(&args.path, namespace, Map::new(), &args.keys)
    };
    let result = result.map_err(|error| CliError::from_property(&args.path, namespace, error))?;

    if args.dry_run || args.print {
        output::json(&Value::Object(result), json_output)?;
    }
    Ok(())
}

fn patch(namespace: &str, json_output: bool, args: PatchArgs) -> Result<(), CliError> {
    let from = match &args.from {
        Some(path) => Some(read_json_object(path)?),
        None => None,
    };

    if args.replace && args.sets.is_empty() && from.is_none() {
        return Err(CliError::usage("--replace requires --set or --from"));
    }
    if args.sets.is_empty() && args.removes.is_empty() && from.is_none() {
        return Err(CliError::usage("patch requires --set, --remove, or --from"));
    }

    for key in &args.removes {
        check_key(key)?;
    }

    let mut values = from.unwrap_or_default();
    for raw in &args.sets {
        let assignment = parse_assignment(raw, args.json_values)?;
        values.insert(assignment.key, assignment.value);
    }
    for key in values.keys() {
        check_key(key)?;
    }

    if args.replace {
        if args.removes.iter().any(|key| values.contains_key(key)) {
            return Err(
                CliError::conflict("cannot set and remove the same property")
                    .with_path(&args.path)
                    .with_namespace(namespace),
            );
        }
        for key in &args.removes {
            values.remove(key);
        }
        let result = if args.dry_run {
            properties::preview_replace(&args.path, namespace, values)
        } else {
            properties::replace(&args.path, namespace, values)
        };
        let result =
            result.map_err(|error| CliError::from_property(&args.path, namespace, error))?;
        if args.dry_run || args.print {
            output::json(&Value::Object(result), json_output)?;
        }
        return Ok(());
    }

    let result = if args.dry_run {
        properties::preview_patch(&args.path, namespace, values, &args.removes)
    } else {
        properties::patch(&args.path, namespace, values, &args.removes)
    };
    let result = result.map_err(|error| CliError::from_property(&args.path, namespace, error))?;
    if args.dry_run || args.print {
        output::json(&Value::Object(result), json_output)?;
    }
    Ok(())
}

fn clear(namespace: &str, args: ClearArgs) -> Result<(), CliError> {
    properties::clear(&args.path, namespace)
        .map_err(|error| CliError::from_property(&args.path, namespace, error))
}

fn check_key(key: &str) -> Result<(), CliError> {
    properties::validate_key(key)
        .map_err(|_| CliError::invalid(format!("invalid property key '{key}'")))
}

struct Assignment {
    key: String,
    value: Value,
}

fn parse_assignment(raw: &str, json_values: bool) -> Result<Assignment, CliError> {
    let Some(equals) = raw.find('=') else {
        return Err(CliError::usage("expected KEY=VALUE or KEY:=JSON"));
    };
    let (left, right) = raw.split_at(equals);
    let right = &right[1..];
    let (key, as_json) = match left.strip_suffix(':') {
        Some(key) => (key, true),
        None => (left, json_values),
    };
    if key.is_empty() {
        return Err(CliError::usage("property key must not be empty"));
    }
    check_key(key)?;
    let value = if as_json {
        serde_json::from_str(right).map_err(|error| {
            CliError::invalid(format!("invalid JSON for property '{key}': {error}"))
        })?
    } else {
        Value::String(right.to_owned())
    };
    Ok(Assignment {
        key: key.to_owned(),
        value,
    })
}

pub(crate) fn read_json_object(path: &Path) -> Result<Map<String, Value>, CliError> {
    let bytes = if path == Path::new("-") {
        read_stdin_limited()?
    } else {
        let metadata = fs::metadata(path).map_err(|error| CliError::input_io(path, error))?;
        if metadata.len() > MAX_ENVELOPE_BYTES as u64 {
            return Err(CliError::invalid("input exceeds 1 MiB").with_path(path));
        }
        fs::read(path).map_err(|error| CliError::input_io(path, error))?
    };
    let value: Value = serde_json::from_slice(&bytes).map_err(|error| {
        CliError::invalid(format!("input is not valid JSON: {error}")).with_path(path)
    })?;
    match value {
        Value::Object(map) => Ok(map),
        _ => Err(CliError::invalid("input must be a JSON object").with_path(path)),
    }
}

fn read_stdin_limited() -> Result<Vec<u8>, CliError> {
    let mut bytes = Vec::new();
    io::stdin()
        .lock()
        .take((MAX_ENVELOPE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| CliError::new(CliErrorKind::Io, format!("cannot read stdin: {error}")))?;
    if bytes.len() > MAX_ENVELOPE_BYTES {
        return Err(CliError::invalid("input exceeds 1 MiB"));
    }
    Ok(bytes)
}

fn scalar_text(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Null => Some("null".to_owned()),
        Value::Array(_) | Value::Object(_) => None,
    }
}
