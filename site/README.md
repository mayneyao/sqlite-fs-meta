# Landing site

Bilingual Astro site for `sqlite-fs-meta`, hosted at
`https://fsmeta.eidos.space`.

The design follows eidos.space: warm paper and ink, IBM Plex Serif headings,
IBM Plex Sans body text, IBM Plex Mono commands, and restrained violet accents.
Sans and Serif Latin fonts are vendored from the official site's Fontsource
5.2.6 assets, with their original licenses in `src/assets/fonts/`.

## Languages

English lives at `/` and Simplified Chinese at `/zh/`. Both are rendered as static
HTML using the same landing page and the typed copy in `src/data/i18n.ts`.
The language link works without JavaScript; with JavaScript it also preserves
the current section anchor. Each page includes its own language, canonical URL,
localized search/social metadata, and alternate-language links.

Translate UI labels and feedback alongside page copy. Keep command syntax,
sample property values, download URLs, and the root-level installer endpoints
the same in both languages.

## Development

```bash
pnpm install
pnpm dev
```

## Build

```bash
pnpm build
pnpm preview
pnpm test
```

## Deploy to Cloudflare

```bash
pnpm dlx wrangler@4.70.0 login
pnpm run deploy
```

Wrangler deploys only `dist/` as static assets to the `fsmeta` Worker and binds
`fsmeta.eidos.space` as its custom domain. Cloudflare manages DNS and TLS for
that binding. No application server or database is required.

Build and verify before deploying. Publish the matching CLI release assets
before changing the public installer channel so installation never points to
an unavailable version. CI builds the site and tests the installers; production
deployment uses the authorized local Wrangler session.

`SITE_URL` defaults to `https://fsmeta.eidos.space` and `SITE_BASE` to `/`.
Installer scripts and the version channel require cache revalidation;
fingerprinted Astro assets use immutable browser caching.

## Agent documentation

`/llms.txt` is the concise documentation index. `/llms-full.txt` contains the
agent guide plus the complete CLI and extension references. The Markdown
endpoints under `/docs/` are statically generated at build time; the two
references import the repository's `docs/cli.md` and `docs/extension.md` as raw
text, so documentation fixes have one source of truth.

The landing page exposes an English or Chinese copyable starter prompt and
links to these endpoints. HTML declares `rel="describedby"` for discovery;
Markdown responses do the same via an HTTP Link header. These are readable
without JavaScript or an application server. Rebuild after editing either
source reference.

## Product demonstration

The Hero tells a three-scene CLI story: attach typed properties with xmeta,
save the folder's fields with schema set, then filter and sort with query. A
scripted terminal, folder tree, property labels, and result table move together.
Visitors can switch scenes, pause, and replay. Commands and results are curated
examples; the demonstration does not execute input or access local files.

Playback starts only when the demo is visible, pauses when the page is hidden,
and respects reduced motion. The first scene is readable without JavaScript.
No database engine or command interpreter is downloaded. English and Chinese
demo copy lives in `src/data/demo.ts`.

The Hero installer switches between macOS, Linux, and Windows. It detects the
browser platform initially and remembers an explicit selection for the session,
including when changing the page language.

## CLI installers

macOS and Linux users can install with:

```bash
curl -fsSL https://fsmeta.eidos.space/install.sh | sh
```

Windows users can run this in PowerShell:

```powershell
irm https://fsmeta.eidos.space/install.ps1 | iex
```

Both scripts resolve `public/cli/latest.txt`, download the native binary and
`SHA256SUMS` from the corresponding GitHub release, verify its checksum and
reported version, then replace the installation. No archive extraction, Rust,
SQLite, or administrator privileges are required. Re-run to update.

The destination defaults to `~/.local/bin`. Unix users whose PATH does not
include that directory receive a PATH reminder. The Windows installer adds it
to the user and current-session PATH; set `XMETA_NO_MODIFY_PATH=1` to disable
that behavior.

For a pinned version or custom directory:

```bash
curl -fsSL https://fsmeta.eidos.space/install.sh | sh -s -- \
  --version 0.6.0 --install-dir "$HOME/.local/bin"
```

The scripts also accept `XMETA_VERSION`, `XMETA_INSTALL_DIR`, `XMETA_LATEST_URL`,
and `XMETA_DOWNLOAD_BASE`. The last two allow private mirrors and isolated tests.
Update `public/cli/latest.txt` and `src/data/project.ts` with each stable release;
the tests check them against the Cargo package version.

Installer tests use a local HTTP fixture and temporary install directories,
including checksum failures and preservation of an existing installation. Unix
tests cover all four supported macOS/Linux targets; Windows tests run the
PowerShell installer on the Windows CI runner.
