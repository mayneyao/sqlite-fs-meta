//! Namespace-level property operations shared by the SQL functions and the CLI.
//!
//! The envelope stored on disk is a JSON object. Property writes are direct
//! xattr/ADS updates; they are not part of SQLite transactions.
use serde_json::{Map, Value};
use std::fs::File;
use std::path::Path;

/// Default attribute namespace, kept in sync with the virtual table default.
pub const DEFAULT_NAMESPACE: &str = "space.eidos.meta";
/// Maximum serialized envelope size accepted by the SQL API.
pub const MAX_ENVELOPE_BYTES: usize = 1024 * 1024;

const MAX_NAMESPACE_BYTES: usize = 255;
const MAX_KEY_BYTES: usize = 1024;

#[derive(Debug, thiserror::Error)]
pub enum PropertyError {
    #[error("Invalid property namespace")]
    InvalidNamespace,
    #[error("Invalid property key")]
    InvalidKey,
    #[error("Cannot set and remove the same property")]
    KeyConflict,
    #[error("Property namespace exceeds 1 MiB")]
    TooLarge,
    #[error("Property data is not valid JSON: {0}")]
    Corrupt(serde_json::Error),
    #[error("Property data could not be encoded: {0}")]
    Encode(serde_json::Error),
    #[error("Property writes reject files with multiple links")]
    HardLinked,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Validate a namespace using the same rules as `fs_meta_read`/`fs_meta_patch`.
pub fn validate_namespace(value: &str) -> Result<(), PropertyError> {
    if value.is_empty()
        || value.len() > MAX_NAMESPACE_BYTES
        || !value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
    {
        return Err(PropertyError::InvalidNamespace);
    }
    Ok(())
}

/// Validate a single top-level property key.
pub fn validate_key(key: &str) -> Result<(), PropertyError> {
    if key.is_empty() || key.len() > MAX_KEY_BYTES || key.contains('\0') {
        return Err(PropertyError::InvalidKey);
    }
    Ok(())
}

/// Read the raw envelope bytes, or `None` when the namespace is absent.
pub fn read_raw(path: &Path, namespace: &str) -> Result<Option<Vec<u8>>, PropertyError> {
    validate_namespace(namespace)?;
    let file = open_target(path, false)?;
    read_bytes(&file, path, namespace)
}

/// Read a namespace as a JSON object. An absent namespace yields an empty map.
pub fn read(path: &Path, namespace: &str) -> Result<Map<String, Value>, PropertyError> {
    parse_envelope(read_raw(path, namespace)?)
}

/// Merge `set` into the existing envelope and remove `remove` keys.
pub fn patch(
    path: &Path,
    namespace: &str,
    set: Map<String, Value>,
    remove: &[String],
) -> Result<Map<String, Value>, PropertyError> {
    let (file, values, bytes) = prepare_patch(path, namespace, set, remove)?;
    write_bytes(&file, path, namespace, &bytes)?;
    Ok(values)
}

/// Compute the result of [`patch`] without writing it.
pub fn preview_patch(
    path: &Path,
    namespace: &str,
    set: Map<String, Value>,
    remove: &[String],
) -> Result<Map<String, Value>, PropertyError> {
    let (_file, values, _bytes) = prepare_patch(path, namespace, set, remove)?;
    Ok(values)
}

/// Replace the namespace with exactly `values`.
pub fn replace(
    path: &Path,
    namespace: &str,
    values: Map<String, Value>,
) -> Result<Map<String, Value>, PropertyError> {
    let (file, bytes) = prepare_replace(path, namespace, &values)?;
    write_bytes(&file, path, namespace, &bytes)?;
    Ok(values)
}

/// Compute the result of [`replace`] without writing it.
pub fn preview_replace(
    path: &Path,
    namespace: &str,
    values: Map<String, Value>,
) -> Result<Map<String, Value>, PropertyError> {
    let (_file, _bytes) = prepare_replace(path, namespace, &values)?;
    Ok(values)
}

/// Remove a namespace. Clearing an absent namespace succeeds.
pub fn clear(path: &Path, namespace: &str) -> Result<(), PropertyError> {
    validate_namespace(namespace)?;
    // Clearing is a write: it changes the inode shared by every hard link.
    let file = open_target(path, true)?;
    remove_bytes(&file, path, namespace)
}

fn open_target(path: &Path, writing: bool) -> Result<File, PropertyError> {
    let file = crate::meta::open_metadata_target(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if writing {
            let metadata = file.metadata()?;
            if metadata.is_file() && metadata.nlink() != 1 {
                return Err(PropertyError::HardLinked);
            }
        }
    }
    #[cfg(not(unix))]
    let _ = writing;
    Ok(file)
}

fn read_bytes(
    file: &File,
    _path: &Path,
    namespace: &str,
) -> Result<Option<Vec<u8>>, PropertyError> {
    #[cfg(unix)]
    {
        use xattr::FileExt;
        Ok(file.get_xattr(crate::meta::attribute_name(namespace).as_ref())?)
    }
    #[cfg(windows)]
    {
        let _ = file;
        Ok(crate::meta::read_envelope(_path, namespace)?)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (file, _path, namespace);
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "OS metadata not supported on this platform",
        )
        .into())
    }
}

fn write_bytes(
    file: &File,
    _path: &Path,
    namespace: &str,
    bytes: &[u8],
) -> Result<(), PropertyError> {
    #[cfg(unix)]
    {
        use xattr::FileExt;
        file.set_xattr(crate::meta::attribute_name(namespace).as_ref(), bytes)?;
    }
    #[cfg(windows)]
    {
        let _ = file;
        crate::meta::restore_envelope(_path, namespace, Some(bytes))?;
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (file, _path, namespace, bytes);
        return Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "OS metadata not supported on this platform",
        )
        .into());
    }
    Ok(())
}

fn remove_bytes(file: &File, _path: &Path, namespace: &str) -> Result<(), PropertyError> {
    #[cfg(unix)]
    {
        use xattr::FileExt;
        match file.remove_xattr(crate::meta::attribute_name(namespace).as_ref()) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            #[cfg(target_os = "macos")]
            Err(error) if error.raw_os_error() == Some(libc::ENOATTR) => Ok(()),
            #[cfg(target_os = "linux")]
            Err(error) if error.raw_os_error() == Some(libc::ENODATA) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
    #[cfg(windows)]
    {
        let _ = file;
        match std::fs::remove_file(crate::meta::stream_path(_path, namespace)?) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (file, _path, namespace);
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "OS metadata not supported on this platform",
        )
        .into())
    }
}

fn parse_envelope(bytes: Option<Vec<u8>>) -> Result<Map<String, Value>, PropertyError> {
    let Some(bytes) = bytes else {
        return Ok(Map::new());
    };
    if bytes.len() > MAX_ENVELOPE_BYTES {
        return Err(PropertyError::TooLarge);
    }
    serde_json::from_slice(&bytes).map_err(PropertyError::Corrupt)
}

fn encode(values: &Map<String, Value>) -> Result<Vec<u8>, PropertyError> {
    let bytes = serde_json::to_vec(values).map_err(PropertyError::Encode)?;
    if bytes.len() > MAX_ENVELOPE_BYTES {
        return Err(PropertyError::TooLarge);
    }
    Ok(bytes)
}

type PreparedPatch = (File, Map<String, Value>, Vec<u8>);

fn prepare_patch(
    path: &Path,
    namespace: &str,
    set: Map<String, Value>,
    remove: &[String],
) -> Result<PreparedPatch, PropertyError> {
    validate_namespace(namespace)?;
    for key in set.keys() {
        validate_key(key)?;
    }
    for key in remove {
        validate_key(key)?;
    }
    if remove.iter().any(|key| set.contains_key(key)) {
        return Err(PropertyError::KeyConflict);
    }

    let file = open_target(path, true)?;
    let mut values = parse_envelope(read_bytes(&file, path, namespace)?)?;
    for key in remove {
        values.remove(key);
    }
    values.extend(set);
    let bytes = encode(&values)?;
    Ok((file, values, bytes))
}

fn prepare_replace(
    path: &Path,
    namespace: &str,
    values: &Map<String, Value>,
) -> Result<(File, Vec<u8>), PropertyError> {
    validate_namespace(namespace)?;
    for key in values.keys() {
        validate_key(key)?;
    }
    let file = open_target(path, true)?;
    let bytes = encode(values)?;
    Ok((file, bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn set(entries: impl IntoIterator<Item = (&'static str, Value)>) -> Map<String, Value> {
        entries
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect()
    }

    #[test]
    fn validates_namespaces_and_keys() {
        assert!(validate_namespace("space.eidos.meta").is_ok());
        assert!(validate_namespace("user.space.eidos.meta").is_ok());
        assert!(validate_namespace("").is_err());
        assert!(validate_namespace("../escape").is_err());
        assert!(validate_namespace("space/eidos").is_err());
        assert!(validate_namespace(&"a".repeat(256)).is_err());

        assert!(validate_key("rating").is_ok());
        assert!(validate_key("").is_err());
        assert!(validate_key("a\0b").is_err());
        assert!(validate_key(&"a".repeat(1025)).is_err());
    }

    #[test]
    fn directory_properties_preserve_values_and_remove_keys() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir(temp.path().join("child")).unwrap();
        let path = temp.path();
        let ns = "space.eidos.test";

        assert!(read(path, ns).unwrap().is_empty());

        let first = patch(
            path,
            ns,
            set([
                ("color", json!("blue")),
                ("details", json!({"keep": true})),
                ("nullable", Value::Null),
            ]),
            &[],
        )
        .unwrap();
        assert_eq!(read(path, ns).unwrap(), first);

        let second = patch(
            path,
            ns,
            set([("color", json!("green"))]),
            &["nullable".to_owned()],
        )
        .unwrap();
        assert_eq!(second.get("color"), Some(&json!("green")));
        assert_eq!(second.get("details"), Some(&json!({"keep": true})));
        assert!(!second.contains_key("nullable"));
        assert!(path.join("child").is_dir());
    }

    #[test]
    fn refuses_corrupt_envelopes_without_replacing_them() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("note.txt");
        std::fs::write(&path, "content").unwrap();
        let ns = "space.eidos.test";

        crate::meta::restore_envelope(&path, ns, Some(b"corrupt")).unwrap();
        assert!(read(&path, ns).is_err());
        assert!(patch(&path, ns, Map::new(), &[]).is_err());
        assert_eq!(
            crate::meta::read_envelope(&path, ns).unwrap(),
            Some(b"corrupt".to_vec())
        );

        // `clear` must work on malformed data.
        clear(&path, ns).unwrap();
        assert_eq!(crate::meta::read_envelope(&path, ns).unwrap(), None);

        // `replace` is the explicit escape hatch.
        crate::meta::restore_envelope(&path, ns, Some(b"corrupt")).unwrap();
        let values = set([("rating", json!(5))]);
        replace(&path, ns, values.clone()).unwrap();
        assert_eq!(read(&path, ns).unwrap(), values);
    }

    #[test]
    fn previews_do_not_write() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("note.txt");
        std::fs::write(&path, "content").unwrap();
        let ns = "space.eidos.test";

        let preview = preview_patch(&path, ns, set([("rating", json!(5))]), &[]).unwrap();
        assert_eq!(preview.get("rating"), Some(&json!(5)));
        assert_eq!(crate::meta::read_envelope(&path, ns).unwrap(), None);

        let replacement = set([("status", json!("Active"))]);
        preview_replace(&path, ns, replacement.clone()).unwrap();
        assert_eq!(crate::meta::read_envelope(&path, ns).unwrap(), None);

        replace(&path, ns, replacement.clone()).unwrap();
        assert_eq!(read(&path, ns).unwrap(), replacement);
    }

    #[test]
    fn rejects_conflicting_updates() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("note.txt");
        std::fs::write(&path, "content").unwrap();
        let result = patch(
            &path,
            "space.eidos.test",
            set([("rating", json!(5))]),
            &["rating".to_owned()],
        );
        assert!(matches!(result, Err(PropertyError::KeyConflict)));
    }

    #[test]
    fn rejects_oversized_envelopes() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("note.txt");
        std::fs::write(&path, "content").unwrap();
        let ns = "space.eidos.test";
        let oversized = "x".repeat(MAX_ENVELOPE_BYTES + 1);
        let values = set([("note", Value::String(oversized))]);
        assert!(matches!(
            replace(&path, ns, values),
            Err(PropertyError::TooLarge)
        ));
        assert_eq!(crate::meta::read_envelope(&path, ns).unwrap(), None);
    }

    #[cfg(unix)]
    #[test]
    fn directories_accept_writes_but_file_hardlinks_and_symlinks_do_not() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join("directory");
        let link = temp.path().join("directory-link");
        std::fs::create_dir(&directory).unwrap();
        std::os::unix::fs::symlink(&directory, &link).unwrap();
        let ns = "space.eidos.test";

        assert!(patch(&directory, ns, set([("color", json!("blue"))]), &[]).is_ok());
        assert!(read(&link, ns).is_err());
        assert!(patch(&link, ns, Map::new(), &[]).is_err());
        assert!(clear(&link, ns).is_err());

        let file = temp.path().join("file.txt");
        let hardlink = temp.path().join("file-link.txt");
        std::fs::write(&file, "content").unwrap();
        std::fs::hard_link(&file, &hardlink).unwrap();
        assert!(read(&file, ns).is_ok());
        assert!(matches!(
            patch(&file, ns, set([("rating", json!(5))]), &[]),
            Err(PropertyError::HardLinked)
        ));
    }
}
