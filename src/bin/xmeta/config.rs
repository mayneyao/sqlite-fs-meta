//! Default namespace discovery, precedence, and persistence.
//!
//! Precedence:
//! 1. `--namespace`
//! 2. `FS_META_NAMESPACE`
//! 3. `--config <FILE>`
//! 4. nearest `.xmeta.json` from the target path (or cwd)
//! 5. user config at `$XDG_CONFIG_HOME/xmeta/config.json` / `%APPDATA%\xmeta\config.json`
//! 6. `space.eidos.meta`
use crate::error::{CliError, CliErrorKind};
use fs_meta::properties::{self, DEFAULT_NAMESPACE};
use serde_json::{Map, Value};
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const ENV_NAMESPACE: &str = "FS_META_NAMESPACE";
pub const PROJECT_FILE: &str = ".xmeta.json";

#[derive(Debug, Clone)]
pub enum Source {
    Cli,
    Env,
    Explicit(PathBuf),
    Project(PathBuf),
    User(PathBuf),
    Default,
}

impl Source {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Cli => "cli",
            Self::Env => "env",
            Self::Explicit(_) => "explicit",
            Self::Project(_) => "project",
            Self::User(_) => "user",
            Self::Default => "default",
        }
    }

    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::Explicit(path) | Self::Project(path) | Self::User(path) => Some(path),
            Self::Cli | Self::Env | Self::Default => None,
        }
    }
}

#[derive(Debug)]
pub struct Resolved {
    pub namespace: String,
    pub source: Source,
    pub user_path: Option<PathBuf>,
    pub project_path: Option<PathBuf>,
    pub explicit_path: Option<PathBuf>,
}

impl Resolved {
    pub fn report(&self) -> Value {
        serde_json::json!({
            "namespace": self.namespace,
            "source": self.source.kind(),
            "source_path": self.source.path().map(path_string),
            "user_config": self.user_path.as_deref().map(path_string),
            "project_config": self.project_path.as_deref().map(path_string),
            "explicit_config": self.explicit_path.as_deref().map(path_string),
        })
    }
}

/// Resolve the namespace for a command.
///
/// `target` is the path being read or written. Project config discovery starts
/// there and walks towards the filesystem root. Config commands pass `None`,
/// which starts discovery at the current working directory.
pub fn resolve(
    cli_namespace: Option<&str>,
    explicit_config: Option<&Path>,
    target: Option<&Path>,
) -> Result<Resolved, CliError> {
    let user_path = user_config_path();
    let project_path = project_config_path(target);
    let explicit_path = explicit_config.map(Path::to_path_buf);

    if let Some(namespace) = cli_namespace {
        validate_cli_namespace(namespace)?;
        return Ok(Resolved {
            namespace: namespace.to_owned(),
            source: Source::Cli,
            user_path,
            project_path,
            explicit_path,
        });
    }

    if let Some(namespace) = env_namespace()? {
        return Ok(Resolved {
            namespace,
            source: Source::Env,
            user_path,
            project_path,
            explicit_path,
        });
    }

    if let Some(path) = explicit_config {
        if !path.is_file() {
            return Err(CliError::not_found("config file does not exist").with_path(path));
        }
        let namespace = load_namespace(path)?.unwrap_or_else(|| DEFAULT_NAMESPACE.to_owned());
        return Ok(Resolved {
            namespace,
            source: Source::Explicit(path.to_path_buf()),
            user_path,
            project_path,
            explicit_path,
        });
    }

    if let Some(path) = &project_path {
        if let Some(namespace) = load_namespace(path)? {
            return Ok(Resolved {
                namespace,
                source: Source::Project(path.clone()),
                user_path,
                project_path,
                explicit_path,
            });
        }
    }

    if let Some(path) = &user_path {
        if let Some(namespace) = load_namespace(path)? {
            return Ok(Resolved {
                namespace,
                source: Source::User(path.clone()),
                user_path,
                project_path,
                explicit_path,
            });
        }
    }

    Ok(Resolved {
        namespace: DEFAULT_NAMESPACE.to_owned(),
        source: Source::Default,
        user_path,
        project_path,
        explicit_path,
    })
}

/// User-level config path, following XDG on Unix and APPDATA on Windows.
pub fn user_config_path() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        if let Some(base) = non_empty_env("APPDATA") {
            return Some(PathBuf::from(base).join("xmeta").join("config.json"));
        }
        if let Some(home) = non_empty_env("USERPROFILE") {
            return Some(
                PathBuf::from(home)
                    .join("AppData")
                    .join("Roaming")
                    .join("xmeta")
                    .join("config.json"),
            );
        }
    }
    #[cfg(not(windows))]
    {
        if let Some(base) = non_empty_env("XDG_CONFIG_HOME") {
            let base = PathBuf::from(base);
            if base.is_absolute() {
                return Some(base.join("xmeta").join("config.json"));
            }
        }
        if let Some(home) = non_empty_env("HOME") {
            return Some(
                PathBuf::from(home)
                    .join(".config")
                    .join("xmeta")
                    .join("config.json"),
            );
        }
    }
    None
}

pub fn user_config_target() -> Result<PathBuf, CliError> {
    user_config_path().ok_or_else(|| {
        CliError::new(
            CliErrorKind::Io,
            "cannot determine the user config directory",
        )
    })
}

/// Nearest existing project config from `target`, falling back to cwd.
pub fn project_config_path(target: Option<&Path>) -> Option<PathBuf> {
    let start = match target {
        Some(path) => {
            let absolute = std::path::absolute(path).ok()?;
            if absolute.is_dir() {
                absolute
            } else {
                absolute.parent()?.to_path_buf()
            }
        }
        None => env::current_dir().ok()?,
    };
    for directory in start.ancestors() {
        let candidate = directory.join(PROJECT_FILE);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Project config path for `config set --project` / `config path --project`.
pub fn project_config_target() -> Result<PathBuf, CliError> {
    let cwd = env::current_dir().map_err(|error| {
        CliError::new(
            CliErrorKind::Io,
            format!("cannot determine the current directory: {error}"),
        )
    })?;
    Ok(project_config_path(Some(&cwd)).unwrap_or_else(|| cwd.join(PROJECT_FILE)))
}

pub fn set_namespace(path: &Path, namespace: &str) -> Result<(), CliError> {
    properties::validate_namespace(namespace)
        .map_err(|_| CliError::invalid(format!("invalid property namespace '{namespace}'")))?;
    let mut config = read_config(path)?.unwrap_or_default();
    config.insert("namespace".to_owned(), Value::String(namespace.to_owned()));
    write_config(path, &config)
}

pub fn unset_namespace(path: &Path) -> Result<(), CliError> {
    let Some(mut config) = read_config(path)? else {
        return Ok(());
    };
    config.remove("namespace");
    if config.is_empty() {
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(config_io(path, "write", error)),
        }
    } else {
        write_config(path, &config)
    }
}

fn env_namespace() -> Result<Option<String>, CliError> {
    match env::var(ENV_NAMESPACE) {
        Ok(value) if value.is_empty() => Ok(None),
        Ok(value) => {
            validate_cli_namespace(&value)?;
            Ok(Some(value))
        }
        Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => Err(CliError::invalid(format!(
            "{ENV_NAMESPACE} is not valid UTF-8"
        ))),
    }
}

fn load_namespace(path: &Path) -> Result<Option<String>, CliError> {
    let Some(config) = read_config(path)? else {
        return Ok(None);
    };
    match config.get("namespace") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(namespace)) => {
            validate_config_namespace(namespace, path)?;
            Ok(Some(namespace.clone()))
        }
        Some(_) => {
            Err(CliError::invalid("config key 'namespace' must be a string").with_path(path))
        }
    }
}

fn validate_cli_namespace(namespace: &str) -> Result<(), CliError> {
    properties::validate_namespace(namespace)
        .map_err(|_| CliError::invalid(format!("invalid property namespace '{namespace}'")))
}

fn validate_config_namespace(namespace: &str, path: &Path) -> Result<(), CliError> {
    properties::validate_namespace(namespace).map_err(|_| {
        CliError::invalid(format!(
            "config contains an invalid namespace '{namespace}'"
        ))
        .with_path(path)
    })
}

fn read_config(path: &Path) -> Result<Option<Map<String, Value>>, CliError> {
    match fs::read_to_string(path) {
        Ok(text) => {
            if text.trim().is_empty() {
                return Ok(Some(Map::new()));
            }
            let value: Value = serde_json::from_str(&text).map_err(|error| {
                CliError::invalid(format!("config is not valid JSON: {error}")).with_path(path)
            })?;
            match value {
                Value::Object(config) => Ok(Some(config)),
                _ => Err(CliError::invalid("config must be a JSON object").with_path(path)),
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(config_io(path, "read", error)),
    }
}

fn write_config(path: &Path, config: &Map<String, Value>) -> Result<(), CliError> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|error| config_io(path, "write", error))?;
        }
    }
    let mut text = serde_json::to_string_pretty(config).map_err(|error| {
        CliError::invalid(format!("cannot serialize config: {error}")).with_path(path)
    })?;
    text.push('\n');
    fs::write(path, text).map_err(|error| config_io(path, "write", error))
}

fn config_io(path: &Path, action: &str, error: io::Error) -> CliError {
    let kind = if error.kind() == io::ErrorKind::NotFound {
        CliErrorKind::NotFound
    } else {
        CliErrorKind::Io
    };
    CliError::new(kind, format!("cannot {action} config: {error}")).with_path(path)
}

fn non_empty_env(key: &str) -> Option<std::ffi::OsString> {
    env::var_os(key).filter(|value| !value.is_empty())
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
