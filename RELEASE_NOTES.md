# sqlite-fs-meta 0.2.4

This patch fixes the creation timestamp reported by filesystem virtual tables.

- `_created_at` now uses the filesystem's actual creation time instead of modification time, for both directory scans and individual file lookups. It returns SQL `NULL` when creation time is unavailable.
- Editing file metadata no longer appears to change the file's creation time on Windows. `_updated_at` and `mtime` continue to report actual filesystem modification time; Windows alternate data stream writes can still change that time.
- Cross-platform regression checks cover creation timestamps, metadata edits and rollback. SQLite CLI integration tests now use extension-enabled SQLite, platform-specific library names and valid Windows paths, and fail explicitly when extension loading is unavailable.

No metadata migration is required. This release does not restore historical filesystem timestamps changed by other programs.
