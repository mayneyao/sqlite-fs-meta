# sqlite-fs-meta 0.4.0

## Directory properties

- Store custom properties on directories using native xattr on macOS/Linux and NTFS alternate data streams on Windows.
- Query directories, including empty folders, alongside regular files in virtual tables. Use `is_dir = 1` to select folders, then update or clear their properties with the same SQL operations and transaction/savepoint rollback as files.
- Use `fs_meta_read` and `fs_meta_patch` on directories, including the virtual table's root. Directory properties belong to the folder itself and are not inherited by its children.
- Discover directory support through the new `directory-properties` capability.
- Directory scans read fresh filesystem metadata so Windows folder modification times match point lookups.

## Compatibility and deletion

Virtual-table scans now include directories by default. Add `WHERE is_dir = 0` to queries that should return only files. The root itself has no table row; access its properties through the scalar functions. Directory rows have an empty `extension` and `NULL` attachment/MIME columns. Their `size` is the native filesystem metadata length, not the total size of their contents.

The default `DELETE` clears only the selected entry's metadata namespace, preserving the folder and its children. With `on_delete='delete_file'`, physical deletion accepts only empty directories and never recursively removes contents. Physical deletion remains irreversible by SQL rollback.

Directory symbolic links and Windows reparse points are rejected, including paths with trailing directory separators. Existing stored properties require no migration.
