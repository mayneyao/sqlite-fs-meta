# sqlite-fs-meta

**Treat your filesystem as a database.**

`sqlite-fs-meta` turns a folder into a SQLite virtual table. Files and directories
become rows; properties such as tags, ratings, and status become typed columns.
Metadata is stored natively in extended attributes (xattr) on macOS/Linux or NTFS
alternate data streams (ADS) on Windows.

No index database. No sync daemon. No sidecar files. The filesystem stays the
source of truth; SQLite is the query engine.

- **SQL over your files**: filter, sort, aggregate, and update files without
  writing traversal scripts.
- **Zero duplication**: metadata lives on the file itself, so there is nothing to
  re-index when files move.
- **Native OS metadata**: xattr / ADS, with no external `attr` tool.
- **Dynamic schema**: declare `tags`, `rating`, `status`, or any other property as
  a column without migrations.
- **Fast point lookups**: `WHERE id = '...'` resolves a file directly instead of
  walking the tree.
- **CLI companion**: `xmeta` handles fast single-target reads and writes.

## Quick start

### Build

```bash
cargo build --release
```

This produces the loadable extension and the native CLI:

- macOS: `target/release/libfs_meta.dylib`, `target/release/xmeta`
- Linux: `target/release/libfs_meta.so`, `target/release/xmeta`
- Windows: `target/release/fs_meta.dll`, `target/release/xmeta.exe`

### Open a folder as a table

```bash
sqlite3 :memory:
```

```sql
.load ./target/release/libfs_meta

CREATE VIRTUAL TABLE files USING fs_meta(
    root = '/path/to/space',
    fields = 'tags TEXT, rating INTEGER, status TEXT'
);

SELECT id, name, size, tags, rating
FROM files
WHERE is_dir = 0 AND extension = 'pdf';
```

### Query and update

```sql
UPDATE files
SET tags = '["design", "urgent"]', rating = 5
WHERE id = 'contracts/2026.pdf';

-- Remove this namespace's metadata; the file itself stays.
DELETE FROM files WHERE id = 'contracts/2026.pdf';
```

Files and directories below `root` are rows. The root itself has no row; use
`fs_meta_read` / `fs_meta_patch` for root properties. See the
[extension reference](docs/extension.md) for the full schema and parameters.

### Use the CLI

`xmeta` is built at `target/release/xmeta`; run `cargo install --path .` to put it
on your `PATH`.

```bash
# KEY=VALUE stores a string; KEY:=JSON stores parsed JSON.
xmeta set note.md status=Active rating:=5 tags:='["review","draft"]'

xmeta get note.md
xmeta get note.md status --text
xmeta patch note.md --set priority=P0 --remove obsolete
xmeta unset note.md obsolete
xmeta clear note.md

printf '%s' '{"owner":"team"}' | xmeta patch ./project --from -
```

`xmeta` does not need SQLite or a database file. See the
[CLI reference](docs/cli.md) for all commands, options, and exit codes.

### Set a default namespace

```bash
# User-wide default.
xmeta config set space.eidos.notes

# Project default: creates or updates the nearest .xmeta.json.
xmeta config set --project space.eidos.notes

xmeta config          # effective namespace, source, and config paths
xmeta config get      # just the effective namespace
xmeta -n space.eidos.cover get note.md   # one-off override
```

Project config is a small JSON object:

```json
{
  "namespace": "space.eidos.notes"
}
```

Resolution order: `--namespace` → `FS_META_NAMESPACE` → `--config` →
nearest `.xmeta.json` → user config → `space.eidos.meta`.

## How it works

```text
SQL ──▶ fs_meta virtual table ──▶ xattr (macOS/Linux)
                                  ADS   (Windows)
```

`SELECT` reads filesystem stats and property envelopes lazily. `UPDATE` merges
values into the file's JSON envelope. `DELETE` clears the namespace. Nothing is
copied into a separate database.

## Safety and limits

- Requires writable xattr / ADS support on the target filesystem.
- Targets must already exist; the CLI never creates files or sidecars.
- Symbolic links, Windows reparse points, and Unix regular files with multiple
  hard links are rejected for writes.
- Directory properties belong to the directory itself and are not inherited.
- Namespaces are limited to 255 bytes; JSON envelopes are limited to 1 MiB.
- Virtual-table writes participate in SQLite transaction rollback. Direct `xmeta`
  writes take effect immediately and require external serialization.

## Documentation

- [Extension reference](docs/extension.md): schema, configuration, scalar
  functions, transactions, and platform details.
- [CLI reference](docs/cli.md): commands, namespace configuration, options, and
  exit codes.

## Tests

```bash
cargo test
```

Runs unit tests and end-to-end integration tests against the system `sqlite3` CLI
and the built `xmeta` binary. Set `SQLITE3_BIN` to an extension-enabled SQLite CLI
if needed. CI covers macOS, Linux, and Windows.
