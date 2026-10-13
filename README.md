# sqlite-fs-meta

A lightweight, zero-sync cross-platform **SQLite Virtual Table extension** that abstracts local files, directories and their operating system extended attributes into a standard SQL table.

- **Zero Duplication**: The filesystem and its metadata are the **Single Source of Truth (SSOT)**. No shadow databases, no background sync daemons.
- **Cross-Platform OS Metadata**:
  - **macOS / Linux**: Stores custom attributes in Extended Attributes (`xattr`).
  - **Windows**: Stores custom attributes in NTFS Alternate Data Streams (`ADS`).
- **Dynamic Schema**: Declare arbitrary custom columns (`tags`, `rating`, `priority`, `notes`) without needing database migrations.
- **Query Optimization**: Point lookups (`WHERE id = '...'` or `WHERE path = '...'`) use `xBestIndex` to access a file or directory in $O(1)$ time without directory traversal.

---

## 1. Quick Start

### Whole-namespace properties

`fs_meta_read(path, namespace)` returns the complete JSON object stored on a regular file or directory; an absent namespace returns `{}`. `fs_meta_patch(path, namespace, set_json, remove_json)` merges an object of values, removes the named keys, preserves other keys and returns the resulting object. JSON null is a stored value. These functions do not require a virtual table and expose properties beyond its declared columns.

```sql
SELECT fs_meta_read('/absolute/note.txt', 'space.eidos.meta');
SELECT fs_meta_patch('/absolute/note.txt', 'space.eidos.meta', '{"rating":4,"tags":["review"]}', '["obsolete"]');

-- A directory, including the root of a virtual table, can have its own properties.
SELECT fs_meta_patch('/absolute/projects', 'space.eidos.meta', '{"owner":"team","status":"Active"}', '[]');
SELECT fs_meta_read('/absolute/projects', 'space.eidos.meta');
```

Namespaces use ASCII letters, digits, `.`, `_` and `-`, up to 255 bytes. JSON envelopes are bounded to 1 MiB, subject to the filesystem's smaller native limits. Corrupt envelopes, entries other than regular files or directories, symbolic links and Windows reparse points are rejected. Unix scalar writes also reject regular files with multiple hard links; a directory's normal link count does not prevent property writes. Hosts must independently validate directory scope. Operations use native xattr on macOS/Linux and ADS on Windows; no command-line `attr` package is used. Calls within one loaded extension are serialized. These scalar writes take effect immediately and are not rolled back by SQL transactions; callers coordinating multiple processes must serialize their writers. Functions are direct-only and cannot run from triggers or views. `fs_meta_capabilities()` includes `metadata-api` and `directory-properties` when these functions and directory support are available.

The underlying filesystem must support writable extended attributes on macOS/Linux or named alternate data streams on Windows. NTFS supports ADS on files and directories; do not assume support on FAT/exFAT or network shares. Unsupported storage and permission failures are reported as errors; the extension does not create sidecar files. Directory properties belong to that directory and are not inherited by its children.

### Build the extension
```bash
cargo build --release
```
This produces the loadable dynamic library:
- macOS: `target/release/libfs_meta.dylib`
- Linux: `target/release/libfs_meta.so`
- Windows: `target/release/fs_meta.dll`

### Test with `sqlite3` CLI
```bash
sqlite3 :memory:
```

```sql
-- 1. Load the extension
.load ./target/release/libfs_meta

-- 2. Create the virtual table for a directory
CREATE VIRTUAL TABLE files USING fs_meta(
    root = '/path/to/my_space',
    fields = 'tags TEXT, rating INTEGER, status TEXT, priority TEXT'
);

-- 3. Query files with standard SQL
SELECT id, name, size, extension, tags, rating FROM files WHERE is_dir = 0 AND extension = 'pdf';

-- 4. Update metadata directly into the file's xattr / ADS
UPDATE files 
SET tags = '["design", "urgent"]', rating = 5, priority = 'P0' 
WHERE id = 'contracts/2026.pdf';

-- 5. Delete metadata
DELETE FROM files WHERE id = 'contracts/2026.pdf';

-- 6. Query and update directory properties with the same table
SELECT id, status FROM files WHERE is_dir = 1;
UPDATE files SET status = 'Active', priority = 'P0' WHERE id = 'contracts';
SELECT id, is_dir, status FROM files WHERE path = 'contracts';
```

Tables recursively include regular files and directories below `root`, including
empty directories. The root itself has no table row; use `fs_meta_read` or
`fs_meta_patch` to access its properties. Use `WHERE is_dir = 0` for file-only
queries. Scans retain the hidden-name and configured ignore rules for directories
and their descendants. Symbolic links and Windows reparse points are excluded
from both scans and point lookups.

Verify on macOS via terminal:
```bash
xattr -l /path/to/my_space/contracts/2026.pdf
# Output: space.eidos.meta: {"priority":"P0","rating":5,"tags":["design","urgent"]}
```

---

## 2. Table Schema

### Built-in System Columns (Read-Only)
| Column | Type | Description |
| :--- | :--- | :--- |
| `id` | `TEXT PRIMARY KEY` | POSIX relative path from `root` (e.g. `docs/spec.pdf`) |
| `name` | `TEXT` | File or directory name (e.g. `spec.pdf` or `docs`) |
| `path` | `TEXT` | Relative path (same as `id`) |
| `size` | `INTEGER` | Native filesystem metadata length (`stat.st_size` on Unix). For a directory, this is not the total size of its contents. |
| `mtime` | `TEXT` | ISO 8601 modification timestamp (UTC) |
| `_created_at` | `TEXT` | Real filesystem creation (birth) timestamp in UTC; `NULL` when unavailable. Never falls back to modification or inode change time. |
| `_updated_at` | `TEXT` | Same filesystem modification timestamp as `mtime`. |
| `extension` | `TEXT` | Lowercase file extension (without dot, e.g. `pdf`); empty string for directories, including names containing dots |
| `is_dir` | `INTEGER` | `0` for regular file, `1` for directory |

The attachment column `file` and its MIME columns `mimetype` / `mime_type` are
`NULL` for directories.

Metadata writes do not change the entry's real creation time. On Windows, writing
an alternate data stream can update `mtime` and `_updated_at`, even when writing
the same metadata value or rolling back a metadata transaction. On macOS and
Linux, xattr writes normally change inode status time instead of `mtime`.
Metadata rollback restores attribute values, not filesystem timestamps.

### User-Defined Custom Columns
Specified in the `fields` argument of `USING fs_meta(...)`:
```sql
CREATE VIRTUAL TABLE my_files USING fs_meta(
    root = '/path/to/dir',
    fields = 'tags TEXT, rating INTEGER, status TEXT, due_date TEXT, notes TEXT'
);
```

---

## 3. Configuration Parameters

| Parameter | Default | Description |
| :--- | :--- | :--- |
| `root` | *(Required)* | Absolute root, or a path relative to the owning database file (including attached databases). In-memory/temporary databases resolve relative roots against the working directory. |
| `fields` | `""` | Comma-separated custom columns (`col_name TYPE, ...`) |
| `namespace` | `"space.eidos.meta"` | The attribute name used for xattr or ADS |
| `ignore` | `node_modules,target,.git,.graft` | Additional comma-separated directory/file ignore names |
| `on_delete` | `"clear_meta"` | `"clear_meta"` (default) clears this namespace's metadata on `DELETE`, retaining the file or directory and its children; `"delete_file"` removes the physical file or an empty directory |

Use `root='.'` to keep a file-backed index portable when its folder moves. SQL
string arguments decode doubled quotes: `root='O''Brien'` addresses `O'Brien`.
`SELECT fs_meta_root_mode()` returns `database` to identify this behavior.

With `on_delete='delete_file'`, deleting a populated directory fails. Physical
deletion never recursively removes a directory's contents and cannot be undone
by SQL rollback. Prefer the default `clear_meta` when deleting properties.

---

## 4. How It Works

```text
       SQL Query (SELECT / UPDATE / DELETE)
                       │
                       ▼
       ┌───────────────────────────────┐
       │   sqlite-fs-meta (vtab)       │
       │   - xBestIndex (O(1) lookup)  │
       │   - Dynamic DDL declaration   │
       └───────────────┬───────────────┘
                       │
         ┌─────────────┴─────────────┐
         ▼                           ▼
┌─────────────────┐         ┌─────────────────┐
│   macOS / Linux │         │     Windows     │
│   xattr::get/set│         │   NTFS ADS Stream
└─────────────────┘         └─────────────────┘
```

1. **`SELECT`**: Iterates through matching files and directories. Standard stats come from the filesystem. Custom columns are lazily read from each entry's extended attributes and returned as typed SQL values.
2. **`UPDATE`**: Intercepts `UPDATE ... SET col = val WHERE id = '...'`. Reads the existing JSON metadata envelope from the file or directory, merges the new values, and writes it directly back to its xattr/ADS. Setting a custom column to SQL `NULL` removes its property key.
3. **`DELETE`**: Clears this namespace's metadata attribute without affecting file contents or directory children (unless `on_delete='delete_file'` is configured).

---

## 5. Running Tests

Virtual-table metadata updates and clears on files and directories participate
in normal transaction/savepoint rollback.
The extension retains exact pre-write envelopes in memory, restoring them on
rollback. This does not provide crash atomicity across SQLite and xattr/ADS or
isolation from other filesystem writers. Opt-in physical file/directory deletion remains
irreversible. `DROP TABLE` with pending metadata writes is rejected; commit or
roll back those writes first. On Linux, unqualified namespaces use `user.`.

`fields` also accepts a JSON array, for example
`fields='[{"name":"Review status","type":"TEXT","key":"review-status"}]'`.
Keep `key` unchanged when renaming the SQL column to preserve attribute values.
The hidden `__fs_meta_remove_key` command column removes a retired key through
the current table's transaction journal after rebuilding its schema.

For on-disk databases, attachment URIs are relative to the database directory,
including the scan-root prefix. Row IDs remain relative to the scan root.
`fs_meta_capabilities()` reports support for these semantics.

```bash
cargo test
```
Runs end-to-end integration tests using the system `sqlite3` CLI tool to verify virtual table creation, file and directory properties, scans and point lookups, transaction rollback, safe physical deletion, and xattr/ADS persistence. Set `SQLITE3_BIN` to an extension-enabled SQLite CLI if the system binary cannot load extensions. CI runs these tests on macOS, Linux and Windows.
