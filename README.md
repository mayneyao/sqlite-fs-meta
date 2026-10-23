<img src="site/public/logo.svg" alt="xmeta logo" width="64" height="64">

# xmeta

**Custom file properties. Manage files like a database.**

`xmeta` is a standalone CLI for reading, writing, and querying file metadata.
SQLite and the existing `fs_meta` extension are bundled, so SQL queries require
no separate installation or database file. The extension can also be embedded
in existing SQLite applications. Files and directories become rows; properties
such as tags, ratings, and status become typed columns.
Metadata is stored natively in extended attributes (xattr) on macOS/Linux or NTFS
alternate data streams (ADS) on Windows.

No index database, sync daemon, or metadata sidecars. The filesystem stays the
source of truth; SQLite is the query engine. Optional CLI configuration uses a
JSON file; file properties and folder schemas stay in xattr / ADS.

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
curl -fsSL https://xmeta.eidos.space/install.sh | sh
```

On Windows, in PowerShell:

```powershell
irm https://xmeta.eidos.space/install.ps1 | iex
```

For an existing film and TV folder (run these commands inside it):

```bash
ls
# Arrival.mkv  Dune.mkv  Her.mkv  Severance.S01E01.mkv

xmeta schema set . "genre TEXT, status TEXT, rating REAL" --json
xmeta set Her.mkv genre=Sci-Fi status=Watched rating:=8.5 --print
xmeta query . "SELECT id, status, rating FROM files WHERE is_dir = 0 AND genre = 'Sci-Fi' AND rating >= 8 ORDER BY rating DESC"
# id       status   rating
# Her.mkv  Watched  8.5

# Machine-readable, parameterized, paged output for agents and scripts.
xmeta query . "SELECT id FROM files WHERE is_dir = 0 AND status = ? ORDER BY id" \
  --params '["Watched"]' --limit 50 --offset 0 --json
```

The rating is a personal example value. Use your own existing files and values;
`xmeta` does not create media files. See the [animated CLI demo](https://xmeta.eidos.space/#demo).

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

To reuse a folder schema defined with `xmeta schema set`, omit `fields`:

```sql
CREATE VIRTUAL TABLE movies USING fs_meta(root = '/path/to/Movies');
```

Explicit `fields` overrides the stored schema. On Windows, load
`./target/release/fs_meta.dll` instead of `./target/release/libfs_meta`.

### Direct property edits

The installers verify the native binary's checksum and version and install to
`~/.local/bin`. Re-run to update. Manual binaries and `SHA256SUMS` are on
[GitHub Releases](https://github.com/mayneyao/xmeta/releases/latest).
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

Eidos Lite provides a general-purpose GUI; its file metadata management feature
requires a [Spark license](https://eidos.space/spark#file-metadata). Eidos Lite and
xmeta share the default `space.eidos.meta` namespace. `xmeta` stays focused on metadata
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
xmeta query ──▶ bundled SQLite + fs_meta ──▶ filesystem stats + xattr / ADS
SQLite app  ──▶ loadable fs_meta         ──▶ the same storage and semantics
```

General queries enumerate the directory tree and collect filesystem stats before
returning rows. Custom property envelopes are read on demand; equality lookups
on `id` can resolve a path directly. Result pagination does not make directory
enumeration incremental, and sorting or aggregation may read metadata across
the whole tree. No persistent index is created.

Through the separate extension, `UPDATE` merges values into the file's JSON
envelope and the default `DELETE` clears the namespace. `xmeta query` is read-only;
use the CLI property commands for edits.

## Safety and limits

- Requires writable xattr / ADS support on the target filesystem.
- Copies, archives, and sync tools may drop native metadata; check preservation
  when transferring files or folder schemas.
- Property commands require existing targets and never create target files.
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
- [Agent guide](https://xmeta.eidos.space/docs/agent-guide.md) and
  [llms.txt](https://xmeta.eidos.space/llms.txt): workflows and documentation discovery.
- [Release notes](RELEASE_NOTES.md): shipped changes and compatibility details.
- [Landing page source](site/): Astro site with local development and deployment
  notes.

## Tests

```bash
cargo test
```

Runs unit tests and end-to-end integration tests against the system `sqlite3` CLI
and the built `xmeta` binary. Set `SQLITE3_BIN` to an extension-enabled SQLite CLI
if needed. CI covers macOS, Linux, and Windows.
