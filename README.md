# sqlite-fs-meta

A lightweight, zero-sync cross-platform **SQLite Virtual Table extension** that abstracts local files and their operating system extended attributes into a standard SQL table.

- **Zero Duplication**: The filesystem and its metadata are the **Single Source of Truth (SSOT)**. No shadow databases, no background sync daemons.
- **Cross-Platform OS Metadata**:
  - **macOS / Linux**: Stores custom attributes in Extended Attributes (`xattr`).
  - **Windows**: Stores custom attributes in NTFS Alternate Data Streams (`ADS`).
- **Dynamic Schema**: Declare arbitrary custom columns (`tags`, `rating`, `priority`, `notes`) without needing database migrations.
- **Query Optimization**: Point lookups (`WHERE id = '...'` or `WHERE path = '...'`) use `xBestIndex` to access single files in $O(1)$ time without directory traversal.

---

## 1. Quick Start

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
SELECT id, name, size, extension, tags, rating FROM files WHERE extension = 'pdf';

-- 4. Update metadata directly into the file's xattr / ADS
UPDATE files 
SET tags = '["design", "urgent"]', rating = 5, priority = 'P0' 
WHERE id = 'contracts/2026.pdf';

-- 5. Delete metadata
DELETE FROM files WHERE id = 'contracts/2026.pdf';
```

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
| `name` | `TEXT` | File name (e.g. `spec.pdf`) |
| `path` | `TEXT` | Relative path (same as `id`) |
| `size` | `INTEGER` | File size in bytes (`stat.st_size`) |
| `mtime` | `TEXT` | ISO 8601 modification timestamp (UTC) |
| `_created_at` | `TEXT` | Real filesystem creation (birth) timestamp in UTC; `NULL` when unavailable. Never falls back to modification or inode change time. |
| `_updated_at` | `TEXT` | Same filesystem modification timestamp as `mtime`. |
| `extension` | `TEXT` | Lowercase file extension (without dot, e.g. `pdf`) |
| `is_dir` | `INTEGER` | `0` for regular file, `1` for directory |

Metadata writes do not change the file's real creation time. On Windows, writing
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
| `on_delete` | `"clear_meta"` | `"clear_meta"` (default) to clear metadata on `DELETE`; `"delete_file"` to remove the physical file |

Use `root='.'` to keep a file-backed index portable when its folder moves. SQL
string arguments decode doubled quotes: `root='O''Brien'` addresses `O'Brien`.
`SELECT fs_meta_root_mode()` returns `database` to identify this behavior.

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

1. **`SELECT`**: Iterates through matching files. Standard file stats come from `fs::metadata`. Custom columns are lazily read from the file's extended attributes and returned as typed SQL values.
2. **`UPDATE`**: Intercepts `UPDATE ... SET col = val WHERE id = '...'`. Reads the existing JSON metadata envelope from the file, merges the new values, and writes it directly back to the file's xattr/ADS.
3. **`DELETE`**: Clears the metadata attribute without affecting file contents (unless `on_delete='delete_file'` is configured).

---

## 5. Running Tests

Metadata updates and clears participate in normal transaction/savepoint rollback.
The extension retains exact pre-write envelopes in memory, restoring them on
rollback. This does not provide crash atomicity across SQLite and xattr/ADS or
isolation from other filesystem writers. Opt-in physical file deletion remains
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
Runs end-to-end integration tests using the system `sqlite3` CLI tool to verify virtual table creation, reads, writes, and xattr persistence.
