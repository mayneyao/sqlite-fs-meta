# Extension Reference

The `fs_meta` module is xmeta's loadable SQLite virtual-table extension. It exposes a
directory tree as a table and stores custom properties in native OS metadata.

## Loading

```bash
cargo build --release
sqlite3 :memory:
```

```sql
.load ./target/release/libfs_meta

CREATE VIRTUAL TABLE files USING fs_meta(
    root = '/path/to/space',
    fields = 'tags TEXT, rating INTEGER, status TEXT, priority TEXT'
);
```

Build artifacts:

- macOS: `libfs_meta.dylib`
- Linux: `libfs_meta.so`
- Windows: `fs_meta.dll`

The loading example above is for macOS/Linux. On Windows, use
`.load ./target/release/fs_meta.dll`.

## Table Schema

### Built-in system columns

| Column        | Type               | Description                                                                                                   |
| :------------ | :----------------- | :------------------------------------------------------------------------------------------------------------ |
| `_id`         | `TEXT PRIMARY KEY` | POSIX relative path from `root` (e.g. `docs/spec.pdf`)                                                        |
| `id`          | `TEXT`             | Alias of `_id`; supports direct path equality lookups.                                                      |
| `name`        | `TEXT`             | File or directory name                                                                                        |
| `path`        | `TEXT`             | Relative path (same as `id`)                                                                                  |
| `size`        | `INTEGER`          | Native `stat.st_size`. For a directory, not the total size of its contents.                                   |
| `mtime`       | `TEXT`             | ISO 8601 modification timestamp (UTC)                                                                         |
| `_created_at` | `TEXT`             | Filesystem creation (birth) timestamp in UTC; `NULL` when unavailable. Never falls back to modification time. |
| `_updated_at` | `TEXT`             | Same filesystem modification timestamp as `mtime`.                                                            |
| `extension`   | `TEXT`             | Lowercase extension without the dot; empty string for directories                                             |
| `is_dir`      | `INTEGER`          | `0` for a regular file, `1` for a directory                                                                   |
| `file`        | `TEXT`             | JSON attachment array with `id`, `name`, `uri`, `size`, and `mediaType`; `NULL` for directories.              |
| `mimetype`    | `TEXT`             | MIME type inferred from the extension; `NULL` for directories.                                               |
| `mime_type`   | `TEXT`             | Alias of `mimetype`.                                                                                        |

The attachment column `file` and its MIME columns `mimetype` / `mime_type` are
`NULL` for directories.

Metadata writes do not change the entry's real creation time. On Windows, writing
an alternate data stream can update `mtime` and `_updated_at`, even when writing
the same value or rolling back a metadata transaction. On macOS and Linux, xattr
writes normally change inode status time instead of `mtime`. Metadata rollback
restores attribute values, not filesystem timestamps.

### Custom columns

```sql
CREATE VIRTUAL TABLE my_files USING fs_meta(
    root = '/path/to/dir',
    fields = 'tags TEXT, rating INTEGER, status TEXT, due_date TEXT, notes TEXT'
);
```

`fields` also accepts a JSON array, separating the SQL column name from the
storage key:

```sql
fields='[{"name":"Review status","type":"TEXT","key":"review-status"}]'
```

When `fields` is omitted, the extension reads the root folder's schema from
`<namespace>.schema` (`space.eidos.meta.schema` by default). The root is resolved
relative to the database before this read. Define the schema once with:

```bash
xmeta schema set ./notes "rating INTEGER, status TEXT"
```

The stored JSON is `{"version":1,"fields":[{"name":"rating","type":"INTEGER"},{"name":"status","type":"TEXT"}]}`.
An optional `key` maps a column to a property key. The CLI and extension share
field validation and metadata conversions; there is no second index or query
implementation. Absent schema means no custom columns. Invalid schema, unknown
versions, duplicate or reserved names, and unsupported types are errors.
Schemas are read when connecting the virtual table; reconnect or recreate the
table after changing declarations. Nested or ancestor schemas are not merged.
Explicit `fields` always wins, including `fields=''` for system columns only.
Changing a schema does not change existing file properties. The namespace plus
`.schema` suffix must fit the native 255-byte namespace limit. Discover this
support through `fs_meta_capabilities()` containing `folder-schema`.

Keep `key` unchanged when renaming the SQL column to preserve attribute values.

The hidden `__fs_meta_remove_key` command column removes a retired key through
the current table's transaction journal after rebuilding its schema.

## Configuration

| Parameter   | Default                           | Description                                                                                                                                                                       |
| :---------- | :-------------------------------- | :-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `root`      | _(Required)_                      | Absolute root, or a path relative to the owning database file (including attached databases). In-memory/temporary databases resolve relative roots against the working directory. |
| `fields`    | Folder schema, otherwise empty    | Comma-separated custom columns (`col_name TYPE, ...`) or a JSON array of `{name,type,key}` objects. Explicit fields override the folder schema, including `fields=''`.            |
| `namespace` | `"space.eidos.meta"`              | Attribute name used for xattr or ADS.                                                                                                                                             |
| `ignore`    | `node_modules,target,.git,.graft` | Additional comma-separated directory/file ignore names.                                                                                                                           |
| `on_delete` | `"clear_meta"`                    | `"clear_meta"` clears this namespace's metadata on `DELETE`, retaining the file/directory and children. `"delete_file"` removes the physical file or an empty directory.          |

Use `root='.'` to keep a file-backed index portable when its folder moves. SQL
string arguments decode doubled quotes: `root='O''Brien'` addresses `O'Brien`.
`SELECT fs_meta_root_mode()` returns `database` to identify this behavior.

With `on_delete='delete_file'`, deleting a populated directory fails. Physical
deletion never recursively removes contents and cannot be undone by SQL rollback.
Prefer the default `clear_meta` when deleting properties.

Tables recursively include regular files and directories below `root`, including
empty directories. The root itself has no row; use `fs_meta_read` or
`fs_meta_patch` for root properties. Use `WHERE is_dir = 0` for file-only queries.
Names starting with `.` and configured ignore names are excluded from general
scans, together with their descendants. Symbolic links and Windows reparse points
are excluded from scans and point lookups.

## Scalar property functions

`fs_meta_read(path, namespace)` returns the complete JSON object stored on a
regular file or directory; an absent namespace returns `{}`.

`fs_meta_patch(path, namespace, set_json, remove_json)` merges an object of values,
removes the named keys, preserves other keys, and returns the resulting object.
JSON null is a stored value. These functions do not require a virtual table and
expose properties beyond its declared columns.

```sql
SELECT fs_meta_read('/absolute/note.txt', 'space.eidos.meta');

SELECT fs_meta_patch(
    '/absolute/note.txt',
    'space.eidos.meta',
    '{"rating":4,"tags":["review"]}',
    '["obsolete"]'
);

-- A directory, including the root of a virtual table, has its own properties.
SELECT fs_meta_patch(
    '/absolute/projects',
    'space.eidos.meta',
    '{"owner":"team","status":"Active"}',
    '[]'
);
```

Rules:

- Namespaces use ASCII letters, digits, `.`, `_`, and `-`, up to 255 bytes.
- JSON envelopes are bounded to 1 MiB, subject to smaller filesystem limits.
- Corrupt envelopes, non-file/non-directory entries, symbolic links, and Windows
  reparse points are rejected.
- Unix scalar writes reject regular files with multiple hard links. A directory's
  normal link count does not prevent property writes.
- Hosts must independently validate directory scope.
- Operations use native xattr on macOS/Linux and ADS on Windows; no command-line
  `attr` package is used.
- Calls within one loaded extension are serialized. Scalar writes take effect
  immediately and are not rolled back by SQL transactions; writers in multiple
  processes must serialize themselves.
- Functions are direct-only and cannot run from triggers or views.

The underlying filesystem must support writable extended attributes on macOS/Linux
or named alternate data streams on Windows. NTFS supports ADS on files and
directories; do not assume support on FAT/exFAT or network shares. Unsupported
storage and permission failures are reported as errors. Directory properties
belong to that directory and are not inherited by children.

`fs_meta_capabilities()` includes `metadata-api` and `directory-properties` when
these functions and directory support are available.

## How it works

```text
       SQL Query (SELECT / UPDATE / DELETE)
                       │
                       ▼
       ┌───────────────────────────────┐
       │   xmeta / fs_meta (vtab)      │
       │   - xBestIndex (O(1) lookup)  │
       │   - Dynamic DDL declaration   │
       └───────────────┬───────────────┘
                       │
         ┌─────────────┴─────────────┐
         ▼                           ▼
┌─────────────────┐         ┌─────────────────┐
│   macOS / Linux │         │     Windows     │
│   xattr::get/set│         │   NTFS ADS      │
└─────────────────┘         └─────────────────┘
```

1. **`SELECT`** iterates matching files and directories. Standard stats come from
   the filesystem; custom columns are lazily read from each entry's metadata.
   General scans first enumerate the whole directory tree and collect entry
   stats in memory. `LIMIT` bounds returned rows, not this initial enumeration.
   Equality lookups on `_id`, `id`, or `path` can resolve an entry directly.
2. **`UPDATE`** reads the existing JSON envelope, merges new values, and writes it
   back to xattr/ADS. Setting a custom column to SQL `NULL` removes its key.
3. **`DELETE`** clears this namespace's metadata without affecting file contents or
   children, unless `on_delete='delete_file'` is configured.

The filesystem remains the source of truth. The virtual table is a live view over
it, not a copy.

Filesystem copies, archives, and sync tools may drop xattr / ADS. Check that file
properties and folder schemas survive transfers with the tools you use.

## Transactions and safety

Virtual-table metadata updates and clears participate in normal
transaction/savepoint rollback. The extension retains exact pre-write envelopes in
memory and restores them on rollback. This does not provide crash atomicity across
SQLite and xattr/ADS or isolation from other filesystem writers. Opt-in physical
deletion is irreversible. `DROP TABLE` with pending metadata writes is rejected;
commit or roll back those writes first. On Linux, unqualified namespaces use
`user.`.

For on-disk databases, attachment URIs are relative to the database directory,
including the scan-root prefix. Row IDs remain relative to the scan root.
`fs_meta_capabilities()` reports support for these semantics.
