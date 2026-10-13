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

fn is_supported_entry(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // Do not expose junctions or other reparse points as ordinary directories.
        if metadata.file_attributes() & 0x400 != 0 {
            return false;
        }
    }
    metadata.is_file() || metadata.is_dir()
}

/// Scans descendants of root, returning regular files and directories.
pub fn scan_directory(root: &Path, ignore_patterns: &[String]) -> Vec<FileInfo> {
    // Strip trailing separators and `.` before inspecting the final entry, so
    // lstat does not resolve a directory symlink due to the path spelling.
    let normalized_root = root.components().collect::<std::path::PathBuf>();
    let root = normalized_root.as_path();
    let mut results = Vec::new();
    let Ok(root_metadata) = root.symlink_metadata() else {
        return results;
    };
    if !root_metadata.is_dir() || !is_supported_entry(&root_metadata) {
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
                && entry.metadata().is_ok_and(|m| is_supported_entry(&m))
        });

    for entry in walker.filter_map(Result::ok) {
        if entry.path() == root {
            continue;
        }
        // WalkDir caches Windows enumeration metadata, which can lag behind
        // directory writes. Use fresh no-follow stats, as point lookups do.
        let metadata = match entry.path().symlink_metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        if !is_supported_entry(&metadata) {
            continue;
        }

        let rel = match entry.path().strip_prefix(root) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let rel_path = to_posix_path(rel);
        if let Some(info) = entry_info(entry.path(), rel_path, &metadata) {
            results.push(info);
        }
    }

    results
}

/// Looks up a single descendant file or directory by relative path safely.
pub fn lookup_file(root: &Path, rel_path: &str) -> Option<FileInfo> {
    let normalized_root = root.components().collect::<std::path::PathBuf>();
    let root = normalized_root.as_path();
    if !is_safe_rel_path(rel_path)
        || !Path::new(rel_path)
            .components()
            .any(|part| matches!(part, std::path::Component::Normal(_)))
    {
        return None;
    }
    let root_metadata = root.symlink_metadata().ok()?;
    if !root_metadata.is_dir() || !is_supported_entry(&root_metadata) {
        return None;
    }
    let target = root.join(rel_path);
    let mut current = root.to_path_buf();
    for part in Path::new(rel_path).components() {
        current.push(part);
        if !is_supported_entry(&current.symlink_metadata().ok()?) {
            return None;
        }
    }
    let metadata = target.symlink_metadata().ok()?;
    if !is_supported_entry(&metadata) {
        return None;
    }

    entry_info(&target, rel_path.to_string(), &metadata)
}

fn entry_info(path: &Path, rel_path: String, metadata: &std::fs::Metadata) -> Option<FileInfo> {
    let is_dir = metadata.is_dir();
    let filename = path.file_name()?.to_string_lossy().to_string();
    let extension = if is_dir {
        String::new()
    } else {
        path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase()
    };
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
        rel_path,
        filename,
        extension,
        size,
        created_at_iso,
        mtime_iso,
        is_dir,
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
    fn directories_and_files_have_consistent_scan_and_lookup_stats() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("photos.png");
        std::fs::create_dir(&directory).unwrap();
        std::fs::write(directory.join("cover.JPG"), b"image").unwrap();
        std::fs::create_dir(root.path().join("empty")).unwrap();

        let scan = scan_directory(root.path(), &[]);
        assert_eq!(scan.len(), 3);
        for path in ["photos.png", "photos.png/cover.JPG", "empty"] {
            let scanned = scan.iter().find(|info| info.rel_path == path).unwrap();
            let lookup = lookup_file(root.path(), path).unwrap();
            let metadata = root.path().join(path).metadata().unwrap();
            assert_eq!(scanned.is_dir, metadata.is_dir());
            assert_eq!(scanned.is_dir, lookup.is_dir);
            assert_eq!(scanned.extension, lookup.extension);
            assert_eq!(scanned.size, metadata.len());
            assert_eq!(scanned.size, lookup.size);
            assert_eq!(scanned.created_at_iso, lookup.created_at_iso);
            assert_eq!(scanned.mtime_iso, lookup.mtime_iso);
            if metadata.is_dir() {
                assert!(scanned.extension.is_empty());
            } else {
                assert_eq!(scanned.extension, "jpg");
            }
        }
        for path in ["", ".", "./", "../", "photos.png/..", "missing"] {
            assert!(lookup_file(root.path(), path).is_none(), "{path}");
        }
    }

    #[test]
    fn ignored_directories_and_their_descendants_are_not_scanned() {
        let root = tempfile::tempdir().unwrap();
        for name in [".hidden", "ignored", "visible"] {
            std::fs::create_dir(root.path().join(name)).unwrap();
            std::fs::write(root.path().join(name).join("note.txt"), b"content").unwrap();
        }
        let scan = scan_directory(root.path(), &["ignored".into()]);
        assert_eq!(scan.len(), 2);
        assert!(scan
            .iter()
            .any(|info| info.rel_path == "visible" && info.is_dir));
        assert!(scan.iter().any(|info| info.rel_path == "visible/note.txt"));
    }

    #[cfg(unix)]
    #[test]
    fn directory_symlinks_are_not_scanned_or_looked_up() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("note.txt"), b"content").unwrap();
        let link = root.path().join("linked");
        std::os::unix::fs::symlink(outside.path(), &link).unwrap();
        assert!(scan_directory(root.path(), &[]).is_empty());
        assert!(lookup_file(root.path(), "linked").is_none());
        assert!(lookup_file(root.path(), "linked/note.txt").is_none());
        for path in [&link, &link.join(""), &link.join(".")] {
            assert!(scan_directory(path, &[]).is_empty());
            assert!(lookup_file(path, "note.txt").is_none());
        }
    }

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
