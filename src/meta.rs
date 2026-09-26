use std::collections::HashMap;
use std::path::Path;

#[cfg(windows)]
fn hold_metadata_file(path: &Path) -> std::io::Result<std::fs::File> {
    use std::os::windows::fs::OpenOptionsExt;
    // ADS writes can create the base file. Open an existing regular file first
    // and deny deletion until the stream operation finishes.
    let file = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0x1 | 0x2) // FILE_SHARE_READ | FILE_SHARE_WRITE
        .custom_flags(0x00200000) // FILE_FLAG_OPEN_REPARSE_POINT
        .open(path)?;
    if !file.metadata()?.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Metadata requires an existing regular file",
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

/// Preserve the exact envelope for transaction rollback, including absence.
pub fn read_envelope(path: &Path, namespace: &str) -> std::io::Result<Option<Vec<u8>>> {
    #[cfg(unix)]
    {
        xattr::get(path, attribute_name(namespace).as_ref())
    }
    #[cfg(windows)]
    {
        let _file = hold_metadata_file(path)?;
        match std::fs::read(format!("{}:{}", path.display(), namespace)) {
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
        xattr::set(path, attribute_name(namespace).as_ref(), bytes)
    }
    #[cfg(windows)]
    {
        let _file = hold_metadata_file(path)?;
        std::fs::write(format!("{}:{}", path.display(), namespace), bytes)
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

/// Writes the metadata envelope (key-value JSON) to the specified file.
pub fn write_metadata(
    path: &Path,
    namespace: &str,
    meta: &HashMap<String, serde_json::Value>,
) -> std::io::Result<()> {
    let bytes = serde_json::to_vec(meta)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    #[cfg(unix)]
    {
        xattr::set(path, attribute_name(namespace).as_ref(), &bytes)
    }

    #[cfg(windows)]
    {
        let _file = hold_metadata_file(path)?;
        let ads_path = format!("{}:{}", path.display(), namespace);
        std::fs::write(&ads_path, &bytes)
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

/// Clears the metadata envelope from the specified file.
pub fn clear_metadata(path: &Path, namespace: &str) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        match xattr::remove(path, attribute_name(namespace).as_ref()) {
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
        let ads_path = format!("{}:{}", path.display(), namespace);
        match std::fs::remove_file(&ads_path) {
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
}
