use chrono::{DateTime, Utc};
use std::path::Path;
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct FileInfo {
    pub rel_path: String,
    pub filename: String,
    pub extension: String,
    pub size: u64,
    /// Real filesystem birth time; unavailable is distinct from modification time.
    pub created_at_iso: Option<String>,
    pub mtime_iso: String,
    pub is_dir: bool,
}

/// Normalizes a path to POSIX forward slashes.
pub fn to_posix_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Checks whether a given relative path attempts to escape the root directory.
pub fn is_safe_rel_path(rel_path: &str) -> bool {
    let path = Path::new(rel_path);
    if path.is_absolute() {
        return false;
    }
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => return false,
            std::path::Component::RootDir | std::path::Component::Prefix(_) => return false,
            _ => {}
        }
    }
    true
}

/// Tests whether a file or directory name should be ignored.
fn should_ignore(name: &str, ignore_patterns: &[String]) -> bool {
    // Always ignore hidden files/directories starting with '.'
    if name.starts_with('.') {
        return true;
    }
    for pattern in ignore_patterns {
        if pattern == name {
            return true;
        }
    }
    false
}

/// Scans the root directory recursively, returning all valid files.
pub fn scan_directory(root: &Path, ignore_patterns: &[String]) -> Vec<FileInfo> {
    let mut results = Vec::new();
    if !root.is_dir() {
        return results;
    }

    let walker = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            if entry.path() == root {
                return true;
            }
            let name = entry.file_name().to_string_lossy();
            !should_ignore(&name, ignore_patterns)
        });

    for entry in walker.filter_map(Result::ok) {
        if entry.path() == root {
            continue;
        }
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let is_dir = metadata.is_dir();
        // Skip directories in list (unless desired), or list them
        if !metadata.is_file() {
            continue;
        }

        let rel = match entry.path().strip_prefix(root) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let rel_path = to_posix_path(rel);
        let filename = entry.file_name().to_string_lossy().to_string();
        let extension = entry
            .path()
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let size = metadata.len();
        let created_at_iso = metadata.created().ok().map(|time| {
            let dt: DateTime<Utc> = time.into();
            dt.to_rfc3339()
        });
        let mtime_iso = metadata
            .modified()
            .ok()
            .map(|t| {
                let dt: DateTime<Utc> = t.into();
                dt.to_rfc3339()
            })
            .unwrap_or_default();

        results.push(FileInfo {
            rel_path,
            filename,
            extension,
            size,
            created_at_iso,
            mtime_iso,
            is_dir,
        });
    }

    results
}

/// Looks up a single file under root by relative path safely.
pub fn lookup_file(root: &Path, rel_path: &str) -> Option<FileInfo> {
    if !is_safe_rel_path(rel_path) {
        return None;
    }
    let target = root.join(rel_path);
    let mut current = root.to_path_buf();
    for part in Path::new(rel_path).components() {
        current.push(part);
        if current.symlink_metadata().ok()?.file_type().is_symlink() {
            return None;
        }
    }
    let metadata = target.symlink_metadata().ok()?;
    if !metadata.is_file() {
        return None;
    }

    let filename = target.file_name()?.to_string_lossy().to_string();
    let extension = target
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let size = metadata.len();
    let created_at_iso = metadata.created().ok().map(|time| {
        let dt: DateTime<Utc> = time.into();
        dt.to_rfc3339()
    });
    let mtime_iso = metadata
        .modified()
        .ok()
        .map(|t| {
            let dt: DateTime<Utc> = t.into();
            dt.to_rfc3339()
        })
        .unwrap_or_default();

    Some(FileInfo {
        rel_path: rel_path.to_string(),
        filename,
        extension,
        size,
        created_at_iso,
        mtime_iso,
        is_dir: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta::{clear_metadata, write_metadata};
    use std::collections::HashMap;
    use std::fs::{File, FileTimes};
    use std::time::{Duration, SystemTime};

    #[test]
    fn creation_time_is_independent_of_mtime_and_metadata_edits() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("probe.txt");
        std::fs::write(&path, b"unchanged").unwrap();
        let created = path.metadata().unwrap().created().ok();
        // Do not backdate mtime: macOS may also move birthtime backwards.
        let modified = SystemTime::now() + Duration::from_secs(86400);
        File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(FileTimes::new().set_modified(modified))
            .unwrap();
        let expected = created.map(|time| DateTime::<Utc>::from(time).to_rfc3339());
        let check = || {
            let scan = scan_directory(root.path(), &[]);
            assert_eq!(scan.len(), 1);
            let lookup = lookup_file(root.path(), "probe.txt").unwrap();
            for info in [&scan[0], &lookup] {
                assert_eq!(info.created_at_iso, expected);
                assert_eq!(
                    info.mtime_iso,
                    DateTime::<Utc>::from(path.metadata().unwrap().modified().unwrap())
                        .to_rfc3339()
                );
            }
            assert_eq!(std::fs::read(&path).unwrap(), b"unchanged");
        };
        check();
        let initial = lookup_file(root.path(), "probe.txt").unwrap();
        assert_ne!(
            initial.created_at_iso.as_deref(),
            Some(initial.mtime_iso.as_str())
        );
        for rating in [1, 2, 2] {
            write_metadata(
                &path,
                "space.eidos.test",
                &HashMap::from([("rating".into(), serde_json::json!(rating))]),
            )
            .unwrap();
            check();
        }
        clear_metadata(&path, "space.eidos.test").unwrap();
        check();
    }
}
