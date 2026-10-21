use fs_meta::properties::PropertyError;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CliErrorKind {
    Io,
    Usage,
    NotFound,
    Invalid,
    Unsupported,
    Conflict,
}

impl CliErrorKind {
    pub fn code(self) -> &'static str {
        match self {
            Self::Io => "io",
            Self::Usage => "usage",
            Self::NotFound => "not_found",
            Self::Invalid => "invalid",
            Self::Unsupported => "unsupported",
            Self::Conflict => "conflict",
        }
    }

    pub fn exit_code(self) -> u8 {
        match self {
            Self::Io => 1,
            Self::Usage => 2,
            Self::NotFound => 3,
            Self::Invalid => 4,
            Self::Unsupported => 5,
            Self::Conflict => 6,
        }
    }
}

#[derive(Debug)]
pub struct CliError {
    pub kind: CliErrorKind,
    pub message: String,
    pub path: Option<PathBuf>,
    pub namespace: Option<String>,
    pub key: Option<String>,
    pub json: bool,
}

impl CliError {
    pub fn new(kind: CliErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            path: None,
            namespace: None,
            key: None,
            json: false,
        }
    }

    pub fn usage(message: impl Into<String>) -> Self {
        Self::new(CliErrorKind::Usage, message)
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(CliErrorKind::Invalid, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(CliErrorKind::NotFound, message)
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(CliErrorKind::Conflict, message)
    }

    pub fn from_property(path: &Path, namespace: &str, error: PropertyError) -> Self {
        let (kind, message) = match &error {
            PropertyError::InvalidNamespace => (
                CliErrorKind::Invalid,
                format!("invalid property namespace '{namespace}'"),
            ),
            PropertyError::InvalidKey => (CliErrorKind::Invalid, "invalid property key".to_owned()),
            PropertyError::KeyConflict => (
                CliErrorKind::Conflict,
                "cannot set and remove the same property".to_owned(),
            ),
            PropertyError::TooLarge => (
                CliErrorKind::Invalid,
                "property namespace exceeds 1 MiB".to_owned(),
            ),
            PropertyError::Corrupt(error) => (
                CliErrorKind::Invalid,
                format!("property data is not valid JSON: {error}"),
            ),
            PropertyError::Encode(error) => (
                CliErrorKind::Invalid,
                format!("property data could not be encoded: {error}"),
            ),
            PropertyError::HardLinked => (
                CliErrorKind::Invalid,
                "property writes reject files with multiple links".to_owned(),
            ),
            PropertyError::Io(error) => classify_io(error),
        };
        Self::new(kind, message)
            .with_path(path)
            .with_namespace(namespace)
    }

    pub fn input_io(path: &Path, error: io::Error) -> Self {
        let (kind, message) = if error.kind() == io::ErrorKind::NotFound {
            (
                CliErrorKind::NotFound,
                "input file does not exist".to_owned(),
            )
        } else {
            classify_io(&error)
        };
        Self::new(kind, message).with_path(path)
    }

    pub fn with_path(mut self, path: &Path) -> Self {
        self.path = Some(path.to_path_buf());
        self
    }

    pub fn with_namespace(mut self, namespace: impl ToString) -> Self {
        self.namespace = Some(namespace.to_string());
        self
    }

    pub fn with_key(mut self, key: impl ToString) -> Self {
        self.key = Some(key.to_string());
        self
    }

    pub fn with_json(mut self, json: bool) -> Self {
        self.json = json;
        self
    }

    pub fn exit_code(&self) -> u8 {
        self.kind.exit_code()
    }
}

fn classify_io(error: &io::Error) -> (CliErrorKind, String) {
    if error.kind() == io::ErrorKind::NotFound {
        return (CliErrorKind::NotFound, "target does not exist".to_owned());
    }
    if error.kind() == io::ErrorKind::PermissionDenied {
        return (CliErrorKind::Io, "permission denied".to_owned());
    }
    if error.kind() == io::ErrorKind::Unsupported || is_unsupported(error) {
        return (
            CliErrorKind::Unsupported,
            "filesystem does not support extended attributes".to_owned(),
        );
    }
    if is_symlink_error(error) {
        return (
            CliErrorKind::Invalid,
            "target must not be a symbolic link".to_owned(),
        );
    }
    if error.kind() == io::ErrorKind::InvalidInput || error.kind() == io::ErrorKind::InvalidData {
        return (CliErrorKind::Invalid, error.to_string());
    }
    (CliErrorKind::Io, error.to_string())
}

fn is_unsupported(error: &io::Error) -> bool {
    #[cfg(unix)]
    if let Some(code) = error.raw_os_error() {
        if code == libc::ENOTSUP || code == libc::EOPNOTSUPP {
            return true;
        }
    }
    #[cfg(windows)]
    if error.raw_os_error() == Some(50) {
        return true;
    }
    let _ = error;
    false
}

fn is_symlink_error(error: &io::Error) -> bool {
    #[cfg(unix)]
    {
        error.raw_os_error() == Some(libc::ELOOP)
    }
    #[cfg(not(unix))]
    {
        let _ = error;
        false
    }
}
