import cliReference from "../../../docs/cli.md?raw";
import extensionReference from "../../../docs/extension.md?raw";
import { agentPrompts } from "./agent";
import { installCommand, repository, siteOrigin, version, windowsInstallCommand } from "./project";

export const agentGuide = `# xmeta Agent Guide

> xmeta ${version} reads and writes properties on existing local files and directories. sqlite-fs-meta is its companion loadable SQLite extension. Metadata lives in native xattr (macOS/Linux) or NTFS ADS (Windows), not in a sidecar or index database.

## Choose the right interface

- Use xmeta for metadata reads, edits, folder schema, and read-only SQL queries. SQLite and the existing fs_meta extension are bundled; no external SQLite installation, .load step, index, or database file is needed.
- Use the separate sqlite-fs-meta extension when integrating filesystem metadata into an existing SQLite application, including SQL updates. Ordinary CLI users can stay with xmeta.
- Use Eidos for visual metadata management. There is no xmeta scan, serve, or embedded Web UI command.
- Read the [CLI reference](${siteOrigin}/docs/cli.md) and [extension reference](${siteOrigin}/docs/extension.md) for the actual flags, SQL behavior, and limits. Do not infer commands from the product name.

## Install and inspect

Check the installed tool before choosing version-specific commands:

\`\`\`sh
xmeta --version
xmeta --help
\`\`\`

macOS / Linux:

\`\`\`sh
${installCommand}
\`\`\`

Windows PowerShell:

\`\`\`powershell
${windowsInstallCommand}
\`\`\`

The installer checks SHA-256 and the binary's reported version before replacing the installation. It installs to ~/.local/bin. Unix users may need to add that directory to PATH; the PowerShell installer adds it to user and current-session PATH. Re-run to update. Manual binaries, the separate extension, and SHA256SUMS are on [GitHub Releases](${repository}/releases/tag/v${version}).

## Work within the requested scope

Establish the existing target paths, fields, values, and whether the task is read-only or a change. Ask for missing details rather than choosing unrelated files or properties. Only perform the requested operations.

For each target, inspect properties first and retain the before values. The namespace can come from --namespace, FS_META_NAMESPACE, an explicit --config file, the nearest trusted .xmeta.json, the user config, or the default space.eidos.meta. Project configuration is resolved from the target path, not just the agent's working directory. To inspect it with xmeta --json config get, run that command in the target directory or a file's parent directory. Pass --namespace explicitly when the task specifies a namespace. Do not change project or user configuration incidentally.

The following examples assume note.md already exists and use the default namespace. Replace the example path, fields, and values with the user's targets. Quote paths and shell arguments; for generated or complex values, pass a JSON object to patch --from rather than interpolate untrusted text into shell syntax.

\`\`\`sh
xmeta --json get "note.md"
xmeta --json set "note.md" status=Active rating:=5 --dry-run
xmeta --json set "note.md" status=Active rating:=5 --print
xmeta --json get "note.md"
\`\`\`

KEY=VALUE stores a string. KEY:=JSON stores typed JSON, including numbers, booleans, arrays, objects, and null. Merge is the default. For example:

\`\`\`sh
xmeta set "note.md" tags:='["review","draft"]' archived:=false
xmeta patch "note.md" --set priority=P0 --remove obsolete --dry-run
printf '%s' '{"owner":"team","archived":false}' | xmeta patch "note.md" --from -
xmeta unset "note.md" obsolete
\`\`\`

Prefer dry-run to preview changes. unset removes only named keys; clear removes the whole namespace and --replace discards its other properties. Use whole-namespace operations only when that is the requested result. Do not overwrite corrupt metadata just to make a command succeed.

## Folder fields and SQL queries

First inspect the selected root with xmeta schema get "<folder>" --json. If the task requests new field declarations, use xmeta schema set "<folder>" "rating INTEGER, status TEXT". This replaces that root's schema, so retain any existing declarations the user still needs. For names containing spaces or keys distinct from column names, pass a versioned JSON schema with --from FILE or --from -.

The schema is a separate <metadata-namespace>.schema attribute on the root directory (space.eidos.meta.schema by default). It contains {"version":1,"fields":[{"name":"rating","type":"INTEGER"}]}. Field types are TEXT, INTEGER, REAL, and BLOB; an optional key maps a column to a stored property key. Schema writes and schema clear never rewrite file properties. Schemas are declarations, not per-file validation rules. Only the query root's schema defines columns; parent and nested schemas are not merged.

\`\`\`sh
xmeta query "./notes" "SELECT id, rating FROM files WHERE rating >= ? ORDER BY rating DESC, id" --params '[4]' --limit 50 --offset 0 --json
\`\`\`

xmeta query defaults to the current folder and, without SQL, lists file paths, sizes, and its custom columns. --json returns columns, rows as objects, limit, offset, and has_more. Use --params with a JSON array for bound SQL values, and --limit / --offset to page results. The default limit is 100, maximum 10000. Use stable ORDER BY on an unchanged folder; filesystem changes between invocations can move page boundaries. General queries enumerate the whole tree and collect filesystem stats before returning rows; pagination does not make enumeration incremental. Custom metadata is read on demand, while sorts and aggregates may read it across the whole tree. Equality lookups on id can resolve a path directly.

query accepts one read-only statement. SQL writes, ATTACH, PRAGMA, extension loading, and fs_meta_patch are blocked. Use xmeta property commands for changes. Column names must be unique; use AS aliases. BLOB results are objects with a $blob hexadecimal string. SQL execution defaults to a 10000 ms progress-handler timeout; it cannot interrupt a native directory scan or filesystem call while it is running. Read the CLI reference for error codes and output details.

## Handle results correctly

- Use --json for compact JSON and JSON-formatted runtime errors. Never confuse an unsuccessful command with an empty object.
- get without keys returns {} for an absent namespace. A missing requested key or --raw on an absent namespace exits 3; selected-key reads produce no partial result.
- Exit codes: 0 success; 1 I/O/permission; 2 usage; 3 missing path/key/explicit config; 4 invalid data/target/namespace/config; 5 unsupported filesystem; 6 conflicting set/remove.
- Re-read after writing and compare the requested values. Report concrete paths, namespace, before/after values, tool version, and failures. File contents must remain unchanged.

## Storage and SQL boundaries

Targets must already exist. Symbolic links and Windows reparse points are excluded; Unix writes to regular files with multiple hard links are rejected. Directory properties belong to the directory itself and are not inherited. JSON envelopes are limited to 1 MiB and may face smaller filesystem limits. Filesystem copies, archives, and sync tools may not preserve xattr/ADS; do not assume metadata survives a transfer without checking the destination.

CLI and scalar-function writes take effect immediately and cannot be undone with a SQL transaction. Virtual-table metadata updates/clears support in-process SQL transaction/savepoint rollback, but do not provide crash atomicity or isolation from other writers. Serialize competing writers. The default SQL DELETE clears namespace metadata; on_delete='delete_file' opts into physical deletion, which cannot be rolled back. Keep the default unless physical deletion was requested.

A virtual table includes entries below its root, not the root itself. Use xmeta on the root directory, or fs_meta_read/fs_meta_patch for its properties. See the extension reference for native loading, typed fields, scan behavior, direct id lookups, and transaction details.

## Starter prompt

${agentPrompts.en}
`;

export const agentDocuments = {
  "agent-guide": agentGuide,
  cli: cliReference,
  extension: extensionReference,
};

export type AgentDocument = keyof typeof agentDocuments;

export const llmsIndex = `# xmeta / sqlite-fs-meta

> Native file metadata for agents and people. xmeta reads and writes properties on existing files; sqlite-fs-meta exposes a folder as a writable SQLite virtual table. Properties live in xattr (macOS/Linux) or NTFS ADS (Windows), with no sidecar or index database.

This site documents release ${version}. Use xmeta --version and --help to check the installed version. xmeta bundles SQLite and fs_meta: use xmeta query for folder SQL and xmeta schema for field declarations stored on the root folder. The separate extension is for SQLite application integration; Eidos provides visual management. Respect target paths and namespaces, inspect before edits, and re-read after writes. query is read-only; CLI metadata writes are immediate, not SQL-transactional.

## Documentation

- [Agent guide](${siteOrigin}/docs/agent-guide.md): Installation, metadata edits, folder schemas, built-in SQL queries, namespaces, read/preview/write/verify workflow, and operating limits.
- [CLI reference](${siteOrigin}/docs/cli.md): Exact xmeta commands, flags, configuration precedence, assignment syntax, exit codes, and limitations.
- [SQLite extension reference](${siteOrigin}/docs/extension.md): Separate extension installation/loading, virtual-table schema, queries, metadata writes, scalar functions, and transaction semantics.
- [Full documentation](${siteOrigin}/llms-full.txt): The agent guide and both references in one plain-text Markdown document.

## Optional

- [Repository](${repository}): Source code, license, and development information.
- [Release ${version}](${repository}/releases/tag/v${version}): Native CLI, separate SQLite extension, and SHA256SUMS for supported platforms.
`;

export const fullAgentDocumentation = `# xmeta / sqlite-fs-meta — Full Documentation

Release: ${version}
Index: ${siteOrigin}/llms.txt

${Object.entries(agentDocuments)
  .map(([name, content]) => `Source: ${siteOrigin}/docs/${name}.md\n\n${content.trim()}`)
  .join("\n\n---\n\n")}
`;
