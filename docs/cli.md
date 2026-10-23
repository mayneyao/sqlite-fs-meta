# CLI Reference

`xmeta` is a standalone binary for reading, writing, and querying file metadata.
It bundles SQLite and the existing `fs_meta` extension, so folder queries require
no external SQLite installation, extension loading, or persistent database.

Use Eidos for visual file metadata management. `xmeta` provides direct property
edits and folder queries without an embedded UI or application server.

## Installation

macOS (Apple Silicon and Intel) and Linux (ARM64 and x86-64):

```bash
curl -fsSL https://fsmeta.eidos.space/install.sh | sh
```

Windows (x86-64), in PowerShell:

```powershell
irm https://fsmeta.eidos.space/install.ps1 | iex
```

The installers download the native binary, verify its checksum and version,
and install to `~/.local/bin`. Re-run to update. On Unix, add that directory to
your shell's `PATH` if it is not already included; the Windows installer adds it
to the user and current-session `PATH`.

Manual downloads and `SHA256SUMS` are available on
[GitHub Releases](https://github.com/mayneyao/sqlite-fs-meta/releases/latest).
To build from source:

```bash
cargo build --release
# macOS/Linux: target/release/xmeta
# Windows:     target/release/xmeta.exe

# Or install it:
cargo install --path . --locked
```

## Commands

| Command                             | Behavior                                                            |
| :---------------------------------- | :------------------------------------------------------------------ |
| `xmeta get <PATH> [KEY]...`         | Read the whole namespace or selected keys.                          |
| `xmeta set <PATH> <KEY=VALUE>...`   | Merge one or more values.                                           |
| `xmeta unset <PATH> <KEY>...`       | Remove one or more keys.                                            |
| `xmeta patch <PATH>`                | Merge `--from` / `--set` / `--remove`, or replace with `--replace`. |
| `xmeta clear <PATH>`                | Remove the entire namespace.                                        |
| `xmeta config`                      | Show or update the default namespace configuration.                 |
| `xmeta schema get/set/clear [PATH]` | Manage the folder's stored field declarations.                      |
| `xmeta query [PATH] [SQL]`          | Query the folder through the bundled SQLite engine.                 |

## Folder schema

```bash
xmeta schema set ./notes "rating INTEGER, status TEXT"
xmeta schema get ./notes --json
xmeta schema clear ./notes
```

The folder must already exist. The schema lives in a separate native attribute
named `<metadata-namespace>.schema` (`space.eidos.meta.schema` by default), not
in the folder's ordinary property object or a sidecar. Linux prefixes native
attributes with `user.`; Windows uses an NTFS alternate data stream. Namespace
resolution uses the same target-based rules as other xmeta commands.

The versioned JSON format is:

```json
{
  "version": 1,
  "fields": [
    { "name": "rating", "type": "INTEGER" },
    { "name": "Review status", "type": "TEXT", "key": "review-status" }
  ]
}
```

Types are `TEXT`, `INTEGER`, `REAL`, and `BLOB`. `key` defaults to `name` and
maps the SQL column to a metadata key. System column names and duplicate names
are rejected. Use `schema set ./notes --from schema.json`, or `--from -` for
stdin, for this JSON format and names containing spaces. `set` replaces only
the schema; `clear` removes only the schema. Neither rewrites file properties
or enforces a type on future property writes. An absent schema has no custom
fields; malformed data or unsupported versions produce an error.

## Query a folder

```bash
xmeta query ./notes
xmeta query ./notes "SELECT id, rating FROM files WHERE rating >= 4 ORDER BY rating DESC"
xmeta query ./notes "SELECT id FROM files WHERE status = ? ORDER BY id" --params '["Active"]' --json
xmeta query ./notes "SELECT id FROM files ORDER BY id" --limit 50 --offset 50 --json
```

`PATH` defaults to `.`. Without `SQL`, xmeta lists file paths, sizes, and the
root folder's custom columns. Each invocation opens a temporary in-memory
SQLite connection, registers the existing extension, and creates `files` for
the requested root. It does not create an index or database file.

Only the query root's schema defines columns for the whole recursive query;
parent and nested folder schemas are not merged. All ordinary extension
system columns, JSON metadata conversions, scan exclusions, and direct `id`
lookups remain available. SQL filtering and sorting run in SQLite. Metadata is
read on demand by the extension, not copied into a second table. Sorting or
aggregating can still require scanning the whole folder.

Queries accept one read-only statement. SQL writes, `ATTACH`, `PRAGMA`,
extension loading, and mutating functions such as `fs_meta_patch` are rejected.
Use `set`, `patch`, `unset`, or `clear` to edit metadata.

`--params` is a JSON array of positional bindings: strings/numbers/null map to
SQLite values, booleans to 0/1, and objects/arrays to JSON text. Integer bindings
must fit signed 64-bit SQLite integers. The default execution timeout is
10,000 milliseconds (`--timeout-ms`); SQLite checks its progress handler between
VM operations, so an individual native filesystem call cannot be interrupted.

Results are paginated after applying SQL: `--limit` defaults to 100 (maximum
10,000), and `--offset` defaults to 0. Use `ORDER BY` for stable pages on an
unchanged folder; concurrent filesystem changes can alter subsequent pages.
Human output is tab-separated, with escaped tabs/newlines in string values.
`--json` emits:

```json
{
  "columns": ["id", "rating"],
  "rows": [{ "id": "note.md", "rating": 5 }],
  "limit": 100,
  "offset": 0,
  "has_more": false
}
```

Column names must be unique; use `AS` aliases for repeated names. SQL NULL
becomes JSON null; BLOB results become `{"$blob":"hexadecimal bytes"}`.
An empty page still includes `columns` and `rows: []`. SQL preparation and
execution errors produce no partial result and use exit code 4 with the
existing JSON error format. Target, namespace, and storage errors retain the
normal CLI exit codes below.

## Reading

```bash
xmeta get note.md
xmeta get note.md status
xmeta get note.md status rating --json
xmeta get note.md status --text
xmeta get note.md --raw
```

- `get <PATH>` prints the full object; an absent namespace prints `{}`.
- `get <PATH> <KEY>` prints the value; multiple keys print an object.
- A missing key exits `3` and produces no partial output.
- `--text` requires at least one key and prints scalar values without JSON
  quoting, one per line.
- `--raw` requires no key and prints the exact stored bytes without a trailing
  newline. An absent namespace exits `3`.
- Output is pretty JSON by default; `--json` selects compact JSON and JSON
  runtime errors.

## Writing

```bash
xmeta set note.md status=Active rating:=5
xmeta set note.md --json-values archived=false
xmeta set note.md --replace status=Active

xmeta patch note.md --set priority=P0 --remove obsolete
printf '%s' '{"owner":"team","archived":false}' | xmeta patch ./project --from -

xmeta unset note.md obsolete
xmeta unset note.md obsolete --strict
xmeta clear note.md
```

- `set` merges by default.
  - `--replace` overwrites the namespace.
  - `--json-values` parses every `KEY=VALUE` value as JSON.
  - `--dry-run` prints the result without writing.
  - `--print` prints the resulting object after a successful write.
- `patch` merges into the current object, applies `--from`, then `--set`, then
  `--remove`.
  - `--replace` starts from an empty object.
  - A key that is both set and removed exits `6`.
  - `--from -` reads a JSON object from stdin.
- `unset` is idempotent by default; `--strict` exits `3` when a key is absent.
- `clear` is idempotent and removes the namespace without deleting the target.

## Assignment syntax

```text
status=Active          # string
rating:=5              # JSON number
archived:=false        # JSON boolean
tags:='["a","b"]'      # JSON array
owner:='{"id":42}'     # JSON object
```

Assignments split at the first `=`. Use `patch --from` when a key contains `=` or
ends with `:`, since those forms are reserved by the assignment syntax.

## Default namespace

Resolution order:

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
xmeta --json config get      # namespace, source, and paths as JSON
xmeta config path            # user config path
xmeta config path --project  # project config path
xmeta config unset           # remove the user-level default
xmeta config unset --project # remove the project-level default
```

A project config is a JSON object:

```json
{
  "namespace": "space.eidos.notes"
}
```

`xmeta config set` preserves other keys in the file. `--config` disables automatic
discovery and is useful for scripts. Project config is discovered from the target
path, so absolute paths work from any working directory; only use project configs
from directories you trust.

## Options

| Option                 | Description                                                  |
| :--------------------- | :----------------------------------------------------------- |
| `-n, --namespace <NS>` | Override the resolved namespace for this invocation.         |
| `--config <FILE>`      | Use an explicit config file and disable automatic discovery. |
| `--json`               | Compact JSON output and JSON-formatted runtime errors.       |
| `--raw`                | Read the exact stored bytes (no key).                        |
| `--text`               | Print scalar values without JSON quoting (requires keys).    |
| `--replace`            | Replace the namespace instead of merging.                    |
| `--json-values`        | Parse every assignment value as JSON.                        |
| `--dry-run`            | Print the result without writing.                            |
| `--print`              | Print the resulting object after a write.                    |
| `--strict`             | Fail when an unset key is absent.                            |
| `--project`            | Target the project config for `xmeta config`.                |

Run `xmeta <command> --help` for command-specific help.

## Exit codes

| Code | Meaning                                      |
| :--- | :------------------------------------------- |
| `0`  | Success                                      |
| `1`  | I/O or permission error                      |
| `2`  | Usage error                                  |
| `3`  | Path, key, or explicit config file not found |
| `4`  | Invalid data, target, namespace, or config   |
| `5`  | Unsupported filesystem                       |
| `6`  | Set/remove conflict                          |

## Safety and limitations

- Direct property writes are single-target. For SQL writes across a folder, use the
  SQLite extension from any SQLite client.
- Writes take effect immediately and are not rolled back by SQL transactions.
- Targets must already exist. The final path component must not be a symbolic link
  or Windows reparse point.
- On Unix, property writes reject regular files with multiple hard links.
- Directory properties belong to the directory itself and are not inherited.
- Namespaces use ASCII letters, digits, `.`, `_`, and `-`, up to 255 bytes.
- JSON envelopes are bounded to 1 MiB.
- Corrupt envelopes are not overwritten by normal patches; use `--replace` or
  `clear` explicitly.
