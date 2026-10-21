# sqlite-fs-meta 0.5.0

## `xmeta` CLI

- Add `xmeta`, a native command-line interface for reading and writing namespace properties. It does not need SQLite, a database file, or an external `attr`/`xattr` package.
- Commands: `get`, `set`, `unset`, `patch`, and `clear`.
- Use `KEY=VALUE` for strings and `KEY:=JSON` for typed values, or pass a JSON object with `patch --from` (including stdin).
- Support `--dry-run`, `--print`, `--replace`, `--json`, `--text`, and `--raw`.
- Exit codes distinguish I/O failures, usage errors, missing paths/keys, invalid data, unsupported filesystems, and set/remove conflicts.
- Configure the default namespace with `xmeta config set`, a project-level `.xmeta.json`, an explicit `--config` file, or `FS_META_NAMESPACE`. Project config is discovered from the target path and takes precedence over the user config.
- CLI and SQL scalar functions now share `src/properties.rs`, so validation, patch semantics, size limits, symlink/hardlink policy, and error messages stay aligned.

## Compatibility

- `fs_meta_read` and `fs_meta_patch` keep their behavior. Their output uses deterministic object key ordering on subsequent writes.
- Existing metadata requires no migration. The CLI reads and writes the same xattr/ADS envelopes.
- The CLI is intentionally single-target in this release; recursive and batch writes remain out of scope.

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
