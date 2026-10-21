# sqlite-fs-meta

**Treat your filesystem as a database.**

`sqlite-fs-meta` exposes a folder as a SQLite virtual table. Every file and
directory becomes a row; declared properties become typed columns. Metadata is
stored natively in extended attributes (xattr) on macOS/Linux or NTFS alternate
data streams (ADS) on Windows.

There is no index database, no sync daemon, and no sidecar file. The filesystem
stays the source of truth; SQLite is the query engine.

- **SQL-first file management**: filter, sort, aggregate, and update files with
  standard SQLite instead of writing traversal scripts.
- **Zero duplication**: metadata lives on the file itself. There is nothing to
  re-index when a file moves, and no stale copy to reconcile.
- **Native OS metadata**: xattr on macOS/Linux, ADS on Windows. No external
  `attr`/`xattr` command is required.
- **Dynamic schema**: declare properties such as `tags`, `rating`, `status`, or
  `priority` as columns without migrations.
- **Fast point lookups**: `WHERE id = '...'` or `WHERE path = '...'` resolves a
  file directly through `xBestIndex` instead of walking the directory tree.
- **Native CLI**: `xmeta` provides scriptable single-target reads and writes
  without SQLite or a database file.

Most file tools import files into a database. `sqlite-fs-meta` does the opposite:
it turns the filesystem itself into the table, so there is nothing to keep in
sync.

---

## 1. Quick Start

### Build

```bash
cargo build --release
```

This produces the loadable extension and the native CLI:

- macOS: `target/release/libfs_meta.dylib` and `target/release/xmeta`
- Linux: `target/release/libfs_meta.so` and `target/release/xmeta`
- Windows: `target/release/fs_meta.dll` and `target/release/xmeta.exe`

### Open a folder as a table

```bash
sqlite3 :memory:
```

```sql
-- 1. Load the extension
.load ./target/release/libfs_meta

-- 2. Expose a directory tree as a virtual table
CREATE VIRTUAL TABLE files USING fs_meta(
    root = '/path/to/my_space',
    fields = 'tags TEXT, rating INTEGER, status TEXT, priority TEXT'
);

-- 3. Query files with standard SQL
SELECT id, name, size, extension, tags, rating
FROM files
WHERE is_dir = 0 AND extension = 'pdf';
```

### Update files with SQL

```sql
-- Update properties; they are written into the file's xattr / ADS envelope.
UPDATE files
SET tags = '["design", "urgent"]', rating = 5, priority = 'P0'
WHERE id = 'contracts/2026.pdf';

-- Delete this namespace's metadata without deleting the file.
DELETE FROM files WHERE id = 'contracts/2026.pdf';

-- Directories are rows too; properties belong to the directory itself.
SELECT id, status FROM files WHERE is_dir = 1;
UPDATE files SET status = 'Active', priority = 'P0' WHERE id = 'contracts';
```

Tables recursively include regular files and directories below `root`, including
empty directories. The root itself has no table row; use `fs_meta_read` or
`fs_meta_patch` for root properties. Use `WHERE is_dir = 0` for file-only queries.
Scans retain the hidden-name and configured ignore rules for directories and their
descendants. Symbolic links and Windows reparse points are excluded from scans and
point lookups.

Verify on macOS via terminal:

```bash
xattr -l /path/to/my_space/contracts/2026.pdf
# Output: space.eidos.meta: {"priority":"P0","rating":5,"tags":["design","urgent"]}
```

### Native CLI (`xmeta`)

The same build also produces `xmeta`. Install it with `cargo install --path .`.
The direct commands write the same JSON envelopes as `fs_meta_patch` and do not
require SQLite or a database file.

```bash
# KEY=VALUE stores a string; KEY:=JSON stores parsed JSON.
xmeta set ~/Documents/note.md status=Active rating:=5 tags:='["review","draft"]'

xmeta get ~/Documents/note.md
xmeta get ~/Documents/note.md rating --text
xmeta get ~/Documents/note.md --raw

xmeta patch ~/Documents/note.md --set priority=P0 --remove obsolete
printf '%s' '{"owner":"team","archived":false}' | xmeta patch ~/Projects --from -

xmeta unset ~/Documents/note.md obsolete
xmeta clear ~/Documents/note.md

# Select a namespace. On Linux, storage uses the user.<namespace> prefix.
xmeta -n space.eidos.cover get ~/Documents/note.md
```

`set` merges by default; add `--replace` to overwrite the namespace. `--dry-run`
prints the result without writing, and `--print` prints the result after a write.
`--json` emits compact JSON and JSON-formatted runtime errors. Run `xmeta --help`
for the full command surface.

### Default namespace

`xmeta` resolves the attribute namespace in this order:

1. `--namespace`
2. `FS_META_NAMESPACE`
3. `--config <FILE>`
4. the nearest `.xmeta.json` from the target path (or the current directory for
   `xmeta config`)
5. the user config: `$XDG_CONFIG_HOME/xmeta/config.json` (default
   `~/.config/xmeta/config.json`) on macOS/Linux, or
   `%APPDATA%\xmeta\config.json` on Windows
6. `space.eidos.meta`

```bash
# User-wide default.
xmeta config set space.eidos.notes

# Project default: creates or updates the nearest .xmeta.json.
xmeta config set --project space.eidos.notes

xmeta config                 # effective namespace, source, and config paths
xmeta config get             # just the effective namespace
xmeta config path            # user config path
xmeta config path --project  # project config path
xmeta config unset           # remove the user-level default
xmeta config unset --project # remove the project-level default
```

A project config is a small JSON object:

```json
{
  "namespace": "space.eidos.notes"
}
```

`xmeta config set` preserves other keys in the file. `--config` disables
automatic discovery and is useful for scripts. Project config is discovered from
the target path, so absolute paths work from any working directory; only use
project configs from directories you trust.

### Advanced: whole-namespace properties

`fs_meta_read(path, namespace)` returns the complete JSON object stored on a
regular file or directory; an absent namespace returns `{}`.
`fs_meta_patch(path, namespace, set_json, remove_json)` merges an object of values,
removes the named keys, preserves other keys and returns the resulting object. JSON
null is a stored value. These functions do not require a virtual table and expose
properties beyond its declared columns.

```sql
SELECT fs_meta_read('/absolute/note.txt', 'space.eidos.meta');
SELECT fs_meta_patch(
    '/absolute/note.txt',
    'space.eidos.meta',
    '{"rating":4,"tags":["review"]}',
    '["obsolete"]'
);

-- A directory, including the root of a virtual table, can have its own properties.
SELECT fs_meta_patch(
    '/absolute/projects',
    'space.eidos.meta',
    '{"owner":"team","status":"Active"}',
    '[]'
);
SELECT fs_meta_read('/absolute/projects', 'space.eidos.meta');
```

Namespaces use ASCII letters, digits, `.`, `_` and `-`, up to 255 bytes. JSON
envelopes are bounded to 1 MiB, subject to the filesystem's smaller native limits.
Corrupt envelopes, entries other than regular files or directories, symbolic links
and Windows reparse points are rejected. Unix scalar writes also reject regular
files with multiple hard links; a directory's normal link count does not prevent
property writes. Hosts must independently validate directory scope.

Operations use native xattr on macOS/Linux and ADS on Windows; no command-line
`attr` package is used. Calls within one loaded extension are serialized. These
scalar writes take effect immediately and are not rolled back by SQL transactions;
callers coordinating multiple processes must serialize their writers. Functions are
direct-only and cannot run from triggers or views. `fs_meta_capabilities()`
includes `metadata-api` and `directory-properties` when these functions and
directory support are available.

The underlying filesystem must support writable extended attributes on
macOS/Linux or named alternate data streams on Windows. NTFS supports ADS on files
and directories; do not assume support on FAT/exFAT or network shares. Unsupported
storage and permission failures are reported as errors; the extension does not
create sidecar files. Directory properties belong to that directory and are not
inherited by its children.

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

`fields` also accepts a JSON array, which separates the SQL column name from the
storage key:

```sql
fields='[{"name":"Review status","type":"TEXT","key":"review-status"}]'
```

Keep `key` unchanged when renaming the SQL column to preserve attribute values.

---

## 3. Configuration Parameters

| Parameter | Default | Description |
| :--- | :--- | :--- |
| `root` | *(Required)* | Absolute root, or a path relative to the owning database file (including attached databases). In-memory/temporary databases resolve relative roots against the working directory. |
| `fields` | `""` | Comma-separated custom columns (`col_name TYPE, ...`) or a JSON array of `{name,type,key}` objects. |
| `namespace` | `"space.eidos.meta"` | The attribute name used for xattr or ADS. |
| `ignore` | `node_modules,target,.git,.graft` | Additional comma-separated directory/file ignore names. |
| `on_delete` | `"clear_meta"` | `"clear_meta"` (default) clears this namespace's metadata on `DELETE`, retaining the file or directory and its children; `"delete_file"` removes the physical file or an empty directory. |

Use `root='.'` to keep a file-backed index portable when its folder moves. SQL
string arguments decode doubled quotes: `root='O''Brien'` addresses `O'Brien`.
`SELECT fs_meta_root_mode()` returns `database` to identify this behavior.

With `on_delete='delete_file'`, deleting a populated directory fails. Physical
deletion never recursively removes a directory's contents and cannot be undone by
SQL rollback. Prefer the default `clear_meta` when deleting properties.

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

1. **`SELECT`**: Iterates through matching files and directories. Standard stats
   come from the filesystem. Custom columns are lazily read from each entry's
   extended attributes and returned as typed SQL values.
2. **`UPDATE`**: Intercepts `UPDATE ... SET col = val WHERE id = '...'`. Reads the
   existing JSON metadata envelope from the file or directory, merges the new
   values, and writes it directly back to its xattr/ADS. Setting a custom column
   to SQL `NULL` removes its property key.
3. **`DELETE`**: Clears this namespace's metadata attribute without affecting file
   contents or directory children (unless `on_delete='delete_file'` is configured).

The filesystem remains the source of truth. The virtual table is a live view over
it, not a copy.

---

## 5. Transactions and Safety

Virtual-table metadata updates and clears on files and directories participate in
normal transaction/savepoint rollback. The extension retains exact pre-write
envelopes in memory, restoring them on rollback. This does not provide crash
atomicity across SQLite and xattr/ADS or isolation from other filesystem writers.
Opt-in physical file/directory deletion remains irreversible. `DROP TABLE` with
pending metadata writes is rejected; commit or roll back those writes first. On
Linux, unqualified namespaces use `user.`.

For on-disk databases, attachment URIs are relative to the database directory,
including the scan-root prefix. Row IDs remain relative to the scan root.
`fs_meta_capabilities()` reports support for these semantics.

The hidden `__fs_meta_remove_key` command column removes a retired key through the
current table's transaction journal after rebuilding its schema.

Direct `xmeta` writes take effect immediately and are not rolled back by SQL
transactions. Targets must already exist. The final path component must not be a
symbolic link or Windows reparse point; on Unix, property writes reject regular
files with multiple hard links. Directory properties belong to the directory
itself and are not inherited.

---

## 6. CLI Reference

`xmeta` is deliberately a single-target property CLI. For SQL queries across a
folder, use the SQLite extension from any SQLite client; the CLI does not embed a
SQL shell.

### Commands

| Command | Behavior |
| :--- | :--- |
| `xmeta get <PATH> [KEY]...` | Read the whole namespace or selected keys. |
| `xmeta set <PATH> <KEY=VALUE>...` | Merge one or more values. |
| `xmeta unset <PATH> <KEY>...` | Remove one or more keys. |
| `xmeta patch <PATH>` | Merge `--from` / `--set` / `--remove`, or replace with `--replace`. |
| `xmeta clear <PATH>` | Remove the entire namespace. |
| `xmeta config` | Show or update the default namespace configuration. |

### Reading

- `get <PATH>` prints the full object; an absent namespace prints `{}`.
- `get <PATH> <KEY>` prints the value; `get <PATH> <K1> <K2>` prints an object.
  A missing key exits `3` and produces no partial output.
- `--text` requires at least one key and prints scalar values without JSON
  quoting, one per line.
- `--raw` requires no key and prints the exact stored bytes without a trailing
  newline. An absent namespace exits `3`.
- Output is pretty JSON by default; `--json` selects compact JSON and JSON
  runtime errors.

### Writing

- `set` merges by default. `--replace` overwrites the namespace;
  `--json-values` parses every `KEY=VALUE` value as JSON; `--dry-run` prints the
  result without writing; `--print` prints the resulting object after a write.
- `patch` merges into the current object, applies `--from`, then `--set`, then
  `--remove`. `--replace` starts from an empty object. A key that is both set and
  removed exits `6`. Use `--from -` to read a JSON object from stdin.
- `unset` is idempotent by default; `--strict` exits `3` when a key is absent.
- `clear` is idempotent and removes the namespace attribute without deleting the
  file or directory.

### Assignment syntax

```text
status=Active          # string
rating:=5              # JSON number
archived:=false        # JSON boolean
tags:='["a","b"]'      # JSON array
owner:='{"id":42}'     # JSON object
```

Assignments split at the first `=`. Use `patch --from` when a key contains `=` or
ends with `:`, since those forms are reserved by the assignment syntax.

### Common options

| Option | Description |
| :--- | :--- |
| `-n, --namespace <NS>` | Override the resolved namespace for this invocation. |
| `--config <FILE>` | Use an explicit config file and disable automatic config discovery. |
| `--json` | Compact JSON output and JSON-formatted runtime errors. |

Per-command options: `--raw`, `--text`, `--replace`, `--json-values`, `--dry-run`,
`--print`, `--strict`, `--set`, `--remove`, `--from`, and `--project`.

### Exit codes

| Code | Meaning |
| :--- | :--- |
| `0` | Success |
| `1` | I/O or permission error |
| `2` | Usage error |
| `3` | Path, key, or explicit config file not found |
| `4` | Invalid data, target, namespace, or config |
| `5` | Unsupported filesystem |
| `6` | Set/remove conflict |

See [Default namespace](#default-namespace) for config precedence and the
`xmeta config` subcommands.

---

## 7. Running Tests

```bash
cargo test
```

Runs end-to-end integration tests using the system `sqlite3` CLI tool to verify
virtual table creation, file and directory properties, scans and point lookups,
transaction rollback, safe physical deletion, and xattr/ADS persistence. It also
runs the `xmeta` CLI integration tests against the built binary. Set `SQLITE3_BIN`
to an extension-enabled SQLite CLI if the system binary cannot load extensions. CI
runs these tests on macOS, Linux and Windows.
