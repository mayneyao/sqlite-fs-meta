# sqlite-fs-meta

**Treat your filesystem as a database.**

`xmeta` is a standalone CLI for reading, writing, and querying file metadata.
SQLite and the existing `fs_meta` extension are bundled, so SQL queries require
no separate installation or database file. The extension can also be embedded
in existing SQLite applications. Files and directories become rows; properties
such as tags, ratings, and status become typed columns.
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
- **One CLI**: `xmeta` handles direct metadata edits, folder schemas, and SQL queries.
- **Folder-local schema**: field declarations live in the root folder's extended
  attributes and are shared by CLI queries and SQLite integrations.

## Quick start

Install on macOS or Linux:

```bash
curl -fsSL https://fsmeta.eidos.space/install.sh | sh
```

On Windows, in PowerShell:

```powershell
irm https://fsmeta.eidos.space/install.ps1 | iex
```

For an existing `notes` folder and `notes/note.md`:

```bash
xmeta set notes/note.md status=Active rating:=5
xmeta schema set ./notes "rating INTEGER, status TEXT"
xmeta query ./notes "SELECT id, rating FROM files WHERE rating >= 4 ORDER BY rating DESC"

# Machine-readable, parameterized, paged output for agents and scripts.
xmeta query ./notes "SELECT id FROM files WHERE status = ? ORDER BY id" \
  --params '["Active"]' --limit 50 --offset 0 --json
```

Schema declarations live in `<metadata-namespace>.schema` on the root folder
(`space.eidos.meta.schema` by default). Updating declarations never rewrites
file properties. Queries use that root's schema across its descendants, without
merging parent or nested schemas. `xmeta query` is read-only; property commands
remain the direct editing interface. See the [CLI reference](docs/cli.md).

### Build from source

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

### Direct property edits

The installers verify the native binary's checksum and version and install to
`~/.local/bin`. Re-run to update. Manual binaries and `SHA256SUMS` are on
[GitHub Releases](https://github.com/mayneyao/sqlite-fs-meta/releases/latest).
For source builds, use `cargo install --path . --locked`.

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

`xmeta` includes SQLite and needs no external SQLite program or database file. See the
[CLI reference](docs/cli.md) for all commands, options, and exit codes.

For visual file metadata management, use Eidos. `xmeta` stays focused on metadata
commands and folder queries; it has no embedded UI or application server.

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
- [Landing page source](site/): Astro site with local development and deployment
  notes.

## Tests

```bash
cargo test
```

Runs unit tests and end-to-end integration tests against the system `sqlite3` CLI
and the built `xmeta` binary. Set `SQLITE3_BIN` to an extension-enabled SQLite CLI
if needed. CI covers macOS, Linux, and Windows.
