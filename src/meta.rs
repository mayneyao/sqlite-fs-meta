use std::collections::HashMap;
use std::{fs::File, path::Path};

/// Opens an existing regular file or directory without following a final link.
/// On Windows, retain the handle throughout ADS operations to deny deletion of
/// the base entry; creating a stream must never recreate a missing base file.
pub(crate) fn open_metadata_target(path: &Path) -> std::io::Result<File> {
    // Trailing separators and `/.` must not let a directory link become an
    // intermediate component that bypasses the no-follow open flags.
    let target = path.components().as_path();
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options
            .share_mode(0x1 | 0x2) // FILE_SHARE_READ | FILE_SHARE_WRITE
            .custom_flags(0x02000000 | 0x00200000); // BACKUP_SEMANTICS | OPEN_REPARSE_POINT
    }
    let file = options.open(target)?;
    let metadata = file.metadata()?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Metadata rejects reparse points",
            ));
        }
    }
    if !metadata.is_file() && !metadata.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Metadata requires an existing regular file or directory",
        ));
    }
    if target.as_os_str() != path.as_os_str() && !metadata.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "A trailing directory separator requires a directory",
        ));
    }
    Ok(file)
}

#[cfg(unix)]
fn attribute_name(namespace: &str) -> std::borrow::Cow<'_, str> {
    #[cfg(target_os = "linux")]
    if !namespace.starts_with("user.") {
        return format!("user.{namespace}").into();
    }
    namespace.into()
}

#[cfg(windows)]
fn stream_path(path: &Path, namespace: &str) -> std::io::Result<std::path::PathBuf> {
    // Canonicalization removes trailing separators from ordinary directories
    // and retains the root separator required for a volume-root stream.
    let mut path = std::fs::canonicalize(path)?.into_os_string();
    path.push(":");
    path.push(namespace);
    Ok(path.into())
}

/// Preserve the exact envelope for transaction rollback, including absence.
pub fn read_envelope(path: &Path, namespace: &str) -> std::io::Result<Option<Vec<u8>>> {
    #[cfg(unix)]
    {
        use xattr::FileExt;
        let file = open_metadata_target(path)?;
        file.get_xattr(attribute_name(namespace).as_ref())
    }
    #[cfg(windows)]
    {
        let _file = open_metadata_target(path)?;
        match std::fs::read(stream_path(path, namespace)?) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (path, namespace);
        Err(std::io::ErrorKind::Unsupported.into())
    }
}

pub fn restore_envelope(path: &Path, namespace: &str, bytes: Option<&[u8]>) -> std::io::Result<()> {
    let Some(bytes) = bytes else {
        return clear_metadata(path, namespace);
    };
    #[cfg(unix)]
    {
        use xattr::FileExt;
        let file = open_metadata_target(path)?;
        file.set_xattr(attribute_name(namespace).as_ref(), bytes)
    }
    #[cfg(windows)]
    {
        let _file = open_metadata_target(path)?;
        std::fs::write(stream_path(path, namespace)?, bytes)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (path, namespace, bytes);
        Err(std::io::ErrorKind::Unsupported.into())
    }
}

/// Reads metadata without silently discarding malformed external data.
pub fn read_metadata(
    path: &Path,
    namespace: &str,
) -> std::io::Result<HashMap<String, serde_json::Value>> {
    match read_envelope(path, namespace)? {
        None => Ok(HashMap::new()),
        Some(bytes) => serde_json::from_slice(&bytes)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error)),
    }
}

/// Writes the metadata envelope (key-value JSON) to a regular file or directory.
pub fn write_metadata(
    path: &Path,
    namespace: &str,
    meta: &HashMap<String, serde_json::Value>,
) -> std::io::Result<()> {
    let bytes = serde_json::to_vec(meta)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    #[cfg(unix)]
    {
        use xattr::FileExt;
        let file = open_metadata_target(path)?;
        file.set_xattr(attribute_name(namespace).as_ref(), &bytes)
    }

    #[cfg(windows)]
    {
        let _file = open_metadata_target(path)?;
        std::fs::write(stream_path(path, namespace)?, &bytes)
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = bytes;
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "OS metadata not supported on this platform",
        ))
    }
}

/// Clears the metadata envelope from a regular file or directory.
pub fn clear_metadata(path: &Path, namespace: &str) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use xattr::FileExt;
        let file = open_metadata_target(path)?;
        match file.remove_xattr(attribute_name(namespace).as_ref()) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            #[cfg(target_os = "macos")]
            Err(e) if e.raw_os_error() == Some(libc::ENOATTR) => Ok(()),
            #[cfg(target_os = "linux")]
            Err(e) if e.raw_os_error() == Some(libc::ENODATA) => Ok(()),
            Err(e) => Err(e),
        }
    }

    #[cfg(windows)]
    {
        let _file = open_metadata_target(path)?;
        match std::fs::remove_file(stream_path(path, namespace)?) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = (path, namespace);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metadata_operations_never_recreate_missing_files() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("missing.txt");
        let namespace = "space.eidos.test";
        assert!(read_envelope(&file, namespace).is_err());
        assert!(restore_envelope(&file, namespace, Some(b"{}")).is_err());
        assert!(write_metadata(&file, namespace, &HashMap::new()).is_err());
        assert!(!file.exists());
    }
    #[test]
    fn envelopes_restore_exact_bytes_and_absence() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("proof.txt");
        std::fs::write(&file, "proof").unwrap();
        let namespace = "space.eidos.test";
        assert_eq!(read_envelope(&file, namespace).unwrap(), None);
        let original = b"{ \"rating\" : 4 }";
        restore_envelope(&file, namespace, Some(original)).unwrap();
        assert_eq!(read_metadata(&file, namespace).unwrap()["rating"], 4);
        assert_eq!(
            read_envelope(&file, namespace).unwrap(),
            Some(original.to_vec())
        );
        restore_envelope(&file, namespace, Some(b"invalid JSON")).unwrap();
        assert!(read_metadata(&file, namespace).is_err());
        restore_envelope(&file, namespace, None).unwrap();
        assert_eq!(read_envelope(&file, namespace).unwrap(), None);
    }

    #[test]
    fn directory_envelopes_restore_and_clear_without_changing_contents() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("folder");
        std::fs::create_dir(&directory).unwrap();
        let child = directory.join("child.txt");
        std::fs::write(&child, "unchanged").unwrap();
        let namespace = "space.eidos.test";
        for path in [root.path(), directory.as_path()] {
            assert_eq!(read_envelope(path, namespace).unwrap(), None);
            let original = b"{ \"rating\" : 4 }";
            restore_envelope(path, namespace, Some(original)).unwrap();
            assert_eq!(read_metadata(path, namespace).unwrap()["rating"], 4);
            assert_eq!(
                read_envelope(path, namespace).unwrap(),
                Some(original.to_vec())
            );

            let metadata = HashMap::from([("color".into(), serde_json::json!("green"))]);
            write_metadata(path, namespace, &metadata).unwrap();
            assert_eq!(read_metadata(path, namespace).unwrap(), metadata);
            restore_envelope(path, namespace, Some(b"invalid JSON")).unwrap();
            assert!(read_metadata(path, namespace).is_err());
            assert_eq!(
                read_envelope(path, namespace).unwrap(),
                Some(b"invalid JSON".to_vec())
            );
            restore_envelope(path, namespace, None).unwrap();
            assert_eq!(read_envelope(path, namespace).unwrap(), None);
            clear_metadata(path, namespace).unwrap();
            assert!(path.is_dir());
        }
        assert_eq!(std::fs::read_to_string(child).unwrap(), "unchanged");
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn metadata_operations_reject_directory_symlinks() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("target");
        let link = root.path().join("link");
        std::fs::create_dir(&directory).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&directory, &link).unwrap();
        #[cfg(windows)]
        if let Err(error) = std::os::windows::fs::symlink_dir(&directory, &link) {
            if error.kind() == std::io::ErrorKind::PermissionDenied {
                return;
            }
            panic!("Could not create directory symlink: {error}");
        }
        let namespace = "space.eidos.test";
        restore_envelope(&directory, namespace, Some(b"{\"rating\":4}")).unwrap();
        for path in [&link, &link.join(""), &link.join(".")] {
            assert!(read_envelope(path, namespace).is_err());
            assert!(restore_envelope(path, namespace, Some(b"{}")).is_err());
            assert!(write_metadata(path, namespace, &HashMap::new()).is_err());
            assert!(clear_metadata(path, namespace).is_err());
        }
        assert_eq!(read_metadata(&directory, namespace).unwrap()["rating"], 4);
    }

    #[cfg(windows)]
    #[test]
    fn directory_streams_accept_trailing_separators() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("");
        let namespace = "space.eidos.test";
        restore_envelope(&path, namespace, Some(b"{\"rating\":4}")).unwrap();
        assert_eq!(read_metadata(root.path(), namespace).unwrap()["rating"], 4);
        clear_metadata(&path, namespace).unwrap();
        assert_eq!(read_envelope(root.path(), namespace).unwrap(), None);
    }
}
