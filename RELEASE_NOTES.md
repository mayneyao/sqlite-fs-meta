# sqlite-fs-meta 0.2.3

This release delivers the changes prepared for 0.2.1 and 0.2.2, whose release validation did not complete successfully.

- Resolve relative scan roots against the owning database directory, including attached databases, and decode escaped SQL string arguments.
- Support JSON field definitions with independent storage keys. Field names may contain spaces, Unicode and quotes; renaming a column can preserve existing attributes.
- Restore exact metadata envelopes on transaction and savepoint rollback. Reject dropping a virtual table with pending metadata writes, and support transactional cleanup of retired keys after schema changes.
- Resolve file attachment URIs relative to the database directory, including nested scan roots. In-memory databases retain scan-root-relative URIs.
- Reject symlink traversal during file lookup, preserve scalar text values and refuse to overwrite malformed metadata.
- Require an existing regular file for Windows metadata operations, preventing rollback from recreating a deleted file through an alternate data stream.
- Qualify default Linux extended attributes with `user.` and ship a Linux arm64 binary alongside macOS arm64/x64, Linux x64 and Windows x64.

`fs_meta_capabilities()` exposes the new storage-key, rollback and attachment-path capabilities. Legacy comma-separated field definitions remain supported.

Rollback journals are in memory. These changes do not provide crash atomicity across SQLite and filesystem attributes, or isolation from independent filesystem writers. Opt-in physical file deletion remains irreversible.
