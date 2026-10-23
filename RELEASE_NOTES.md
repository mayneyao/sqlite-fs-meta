# xmeta 0.6.0

## xmeta queries and folder schemas

- Bundle SQLite and the existing `fs_meta` extension in `xmeta`. `xmeta query [PATH] [SQL]` queries a folder without a separate SQLite installation, `.load` step, index, or database file.
- Add `xmeta schema get/set/clear`. Versioned field declarations are stored in the root folder's `<metadata-namespace>.schema` xattr or NTFS ADS. Schema edits leave file properties unchanged.
- The extension reads folder schemas when `fields` is omitted. Explicit fields, including `fields=''`, take precedence. CLI and application integrations share the same scanner, column conversions, and query behavior.
- CLI queries are read-only and support bound JSON parameters, paged JSON/TSV results, and a SQLite progress-handler timeout. SQL writes, extension loading, and mutating scalar functions are blocked.
- Existing file metadata and direct read/write commands require no migration. No Web UI or application runtime is added to the CLI.

# xmeta 0.5.0

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

# xmeta 0.4.0

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
