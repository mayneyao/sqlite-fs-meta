use std::collections::HashMap;
use std::path::Path;

/// Reads the metadata envelope (key-value JSON) from the specified file.
pub fn read_metadata(path: &Path, namespace: &str) -> std::io::Result<HashMap<String, serde_json::Value>> {
    #[cfg(unix)]
    {
        match xattr::get(path, namespace)? {
            Some(bytes) => {
                let map: HashMap<String, serde_json::Value> =
                    serde_json::from_slice(&bytes).unwrap_or_default();
                Ok(map)
            }
            None => Ok(HashMap::new()),
        }
    }

    #[cfg(windows)]
    {
        let ads_path = format!("{}:{}", path.display(), namespace);
        match std::fs::read(&ads_path) {
            Ok(bytes) => {
                let map: HashMap<String, serde_json::Value> =
                    serde_json::from_slice(&bytes).unwrap_or_default();
                Ok(map)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(HashMap::new()),
            Err(e) => Err(e),
        }
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = (path, namespace);
        Ok(HashMap::new())
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
        xattr::set(path, namespace, &bytes)
    }

    #[cfg(windows)]
    {
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
        match xattr::remove(path, namespace) {
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
