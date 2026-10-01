# sqlite-fs-meta 0.3.0

## What's new

- Read a file's complete metadata namespace with `fs_meta_read(path, namespace)`, including properties outside a virtual table's declared columns.
- Update selected properties with `fs_meta_patch(path, namespace, set_json, remove_json)`. Unchanged keys are preserved, JSON null remains a value, and named keys can be removed explicitly.
- Discover these functions through the `metadata-api` capability. They use native extended attributes on macOS/Linux and alternate data streams on Windows.

Existing virtual tables remain compatible and require no metadata migration. Namespace operations take effect immediately, outside SQL transaction rollback; applications coordinating multiple processes must serialize writers. Hosts remain responsible for validating file access scope.
