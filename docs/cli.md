# CLI Reference

`xmeta` is a standalone binary for reading and writing namespace properties. It
does not require SQLite or a database file; it writes the same JSON envelopes as
`fs_meta_patch`.

Use Eidos for visual file metadata management. `xmeta` provides single-target
property reads and writes without an embedded UI or application runtime.

Prebuilt binaries for macOS (Apple Silicon and Intel), Linux (ARM64 and x86-64),
and Windows (x86-64) are available on
[GitHub Releases](https://github.com/mayneyao/sqlite-fs-meta/releases/latest).
Download the `xmeta-<version>-<target>` archive, verify it against `SHA256SUMS`,
extract it, and place `xmeta` (`xmeta.exe` on Windows) on your `PATH`.

```bash
cargo build --release
# macOS/Linux: target/release/xmeta
# Windows:     target/release/xmeta.exe

# Or install it:
cargo install --path .
```

## Commands

| Command | Behavior |
| :--- | :--- |
| `xmeta get <PATH> [KEY]...` | Read the whole namespace or selected keys. |
| `xmeta set <PATH> <KEY=VALUE>...` | Merge one or more values. |
| `xmeta unset <PATH> <KEY>...` | Remove one or more keys. |
| `xmeta patch <PATH>` | Merge `--from` / `--set` / `--remove`, or replace with `--replace`. |
| `xmeta clear <PATH>` | Remove the entire namespace. |
| `xmeta config` | Show or update the default namespace configuration. |

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

| Option | Description |
| :--- | :--- |
| `-n, --namespace <NS>` | Override the resolved namespace for this invocation. |
| `--config <FILE>` | Use an explicit config file and disable automatic discovery. |
| `--json` | Compact JSON output and JSON-formatted runtime errors. |
| `--raw` | Read the exact stored bytes (no key). |
| `--text` | Print scalar values without JSON quoting (requires keys). |
| `--replace` | Replace the namespace instead of merging. |
| `--json-values` | Parse every assignment value as JSON. |
| `--dry-run` | Print the result without writing. |
| `--print` | Print the resulting object after a write. |
| `--strict` | Fail when an unset key is absent. |
| `--project` | Target the project config for `xmeta config`. |

Run `xmeta <command> --help` for command-specific help.

## Exit codes

| Code | Meaning |
| :--- | :--- |
| `0` | Success |
| `1` | I/O or permission error |
| `2` | Usage error |
| `3` | Path, key, or explicit config file not found |
| `4` | Invalid data, target, namespace, or config |
| `5` | Unsupported filesystem |
| `6` | Set/remove conflict |

## Safety and limitations

- The CLI is deliberately single-target. For SQL queries across a folder, use the
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
