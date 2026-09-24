use chrono::{DateTime, Utc};
use std::path::Path;
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct FileInfo {
    pub rel_path: String,
    pub filename: String,
    pub extension: String,
    pub size: u64,
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
        if is_dir {
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
    let metadata = target.metadata().ok()?;
    if metadata.is_dir() {
        return None;
    }

    let filename = target.file_name()?.to_string_lossy().to_string();
    let extension = target
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let size = metadata.len();
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
        mtime_iso,
        is_dir: false,
    })
}
