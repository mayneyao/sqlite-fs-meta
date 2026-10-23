use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::OnceLock;
use tempfile::tempdir;

#[test]
fn extension_reads_folder_schema_and_explicit_fields_still_override() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("notes");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("note.md"), "content").unwrap();
    fs_meta::properties::patch(
        &root.join("note.md"),
        "space.eidos.meta",
        serde_json::from_str(r#"{"rating":5,"status":"Active"}"#).unwrap(),
        &[],
    )
    .unwrap();
    fs_meta::folder_schema::write(
        &root,
        "space.eidos.meta",
        fs_meta::folder_schema::FolderSchema::from_fields("rating INTEGER, status TEXT").unwrap(),
    )
    .unwrap();
    let db = temp.path().join("catalog.sqlite");
    let sql = "CREATE VIRTUAL TABLE files USING fs_meta(root='notes'); SELECT id, rating, status FROM files;";
    assert_eq!(run_sqlite(temp.path(), &db, sql), "note.md|5|Active");
    // A new connection resolves the same root-relative schema from the DB path.
    assert_eq!(
        run_sqlite(&std::env::temp_dir(), &db, "SELECT rating FROM files;"),
        "5"
    );
    fs_meta::properties::replace(
        &root,
        "space.eidos.meta.schema",
        serde_json::from_str(r#"{"version":99,"fields":[]}"#).unwrap(),
    )
    .unwrap();
    let quoted = sql_string(root.to_str().unwrap());
    assert_eq!(run_sqlite(temp.path(), Path::new(":memory:"), &format!("CREATE VIRTUAL TABLE files USING fs_meta(root={quoted}, fields='status TEXT'); SELECT status FROM files;")), "Active");
    let output = run_sqlite_output(
        temp.path(),
        Path::new(":memory:"),
        &format!("CREATE VIRTUAL TABLE files USING fs_meta(root={quoted});"),
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("schema"));
}

fn extension_library() -> &'static Path {
    static LIBRARY: OnceLock<PathBuf> = OnceLock::new();
    LIBRARY.get_or_init(|| {
        assert!(Command::new(env!("CARGO"))
            .args(["build", "--locked"])
            .status()
            .expect("Failed to build test extension")
            .success());
        let library = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join(format!(
                "{}fs_meta{}",
                std::env::consts::DLL_PREFIX,
                std::env::consts::DLL_SUFFIX
            ));
        assert!(
            library.is_file(),
            "Missing extension: {}",
            library.display()
        );
        library
    })
}

fn sql_string(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn run_sqlite_output(cwd: &Path, database: &Path, sql: &str) -> Output {
    let library = extension_library();
    let executable = std::env::var_os("SQLITE3_BIN").unwrap_or_else(|| "sqlite3".into());
    let mut child = Command::new(&executable)
        .current_dir(cwd)
        .arg("-bail")
        .arg(database)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| {
            panic!("Cannot run {executable:?}: {error}. Set SQLITE3_BIN to an extension-enabled SQLite CLI.")
        });
    writeln!(
        child.stdin.take().unwrap(),
        "SELECT load_extension({}, 'sqlite3_fsmeta_init');\n{sql}",
        sql_string(&library.to_string_lossy())
    )
    .expect("Failed to send SQL to SQLite CLI");
    child
        .wait_with_output()
        .expect("Failed to wait for SQLite CLI")
}

fn run_sqlite(cwd: &Path, database: &Path, sql: &str) -> String {
    let output = run_sqlite_output(cwd, database, sql);
    assert!(
        output.status.success(),
        "SQLite CLI failed (extension loading is required): {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn assert_sqlite_error(cwd: &Path, sql: &str) {
    let output = run_sqlite_output(cwd, Path::new(":memory:"), sql);
    assert!(
        !output.status.success(),
        "SQLite unexpectedly accepted SQL: {sql}"
    );
    assert!(
        !output.stderr.is_empty(),
        "SQLite failed without reporting an error"
    );
}

#[test]
fn relative_roots_follow_database_moves_and_decode_quoted_paths() {
    let temp = tempdir().unwrap();
    // Keep SQL quote/space coverage on Windows; double quotes are invalid there.
    let root = temp.path().join(if cfg!(windows) {
        "O'Brien notes"
    } else {
        "O'Brien \"notes\""
    });
    fs::create_dir(&root).unwrap();
    fs::write(root.join("proof.txt"), "proof").unwrap();
    let run = |db: &Path, sql: &str| run_sqlite(temp.path(), db, sql);
    let file = root.join("files.sqlite");
    assert_eq!(run(&file, "CREATE VIRTUAL TABLE files USING fs_meta(root='.', fields='rating INTEGER'); SELECT count(*) FROM files WHERE name='proof.txt';"), "1");
    let absolute_root = root.to_str().unwrap().replace('\'', "''");
    assert_eq!(run(std::path::Path::new(":memory:"), &format!("CREATE VIRTUAL TABLE files USING fs_meta(root='{absolute_root}'); SELECT count(*) FROM files WHERE name='proof.txt';")), "1");
    let moved = temp.path().join("moved");
    fs::rename(&root, &moved).unwrap();
    let file = moved.join("files.sqlite");
    assert_eq!(run(&file, "UPDATE files SET rating=7 WHERE name='proof.txt'; SELECT rating FROM files WHERE name='proof.txt';"), "7");
    let attached = file.to_str().unwrap().replace('\'', "''");
    assert_eq!(run(std::path::Path::new(":memory:"), &format!("ATTACH '{attached}' AS other; SELECT rating FROM other.files WHERE name='proof.txt';")), "7");
    assert_eq!(run(&file, "BEGIN; UPDATE files SET rating=1 WHERE _id='proof.txt'; ROLLBACK; SELECT rating FROM files WHERE _id='proof.txt';"), "7");
    assert_eq!(run(&file, "BEGIN; UPDATE files SET rating=2 WHERE _id='proof.txt'; SAVEPOINT inner; UPDATE files SET rating=3 WHERE _id='proof.txt'; ROLLBACK TO inner; RELEASE inner; COMMIT; SELECT rating FROM files WHERE _id='proof.txt';"), "2");
    assert_eq!(
        run(
            &file,
            r#"DROP TABLE files; CREATE VIRTUAL TABLE files USING fs_meta(root='.', fields='[{"name":"Owner''s rating","type":"INTEGER","key":"rating"}]'); SELECT "Owner's rating" FROM files WHERE _id='proof.txt';"#
        ),
        "2"
    );
}

#[test]
fn namespace_properties_preserve_json_and_interoperate_with_virtual_tables() {
    let temp = tempdir().unwrap();
    let file = temp.path().join("note.txt");
    fs::write(&file, "content").unwrap();
    let file = sql_string(&file.to_string_lossy());
    let root = sql_string(&temp.path().to_string_lossy());
    let sql = format!(
        r#"
SELECT fs_meta_read({file}, 'space.eidos.test');
SELECT json_extract(fs_meta_patch({file}, 'space.eidos.test', '{{"rating":4,"unknown":{{"nested":true}},"old":1}}', '[]'), '$.rating');
CREATE VIRTUAL TABLE files USING fs_meta(root={root}, namespace='space.eidos.test', fields='rating INTEGER');
SELECT rating FROM files WHERE _id='note.txt';
UPDATE files SET rating=5 WHERE _id='note.txt';
SELECT json_extract(fs_meta_patch({file}, 'space.eidos.test', '{{"nullable":null}}', '["old"]'), '$.rating');
SELECT json_extract(fs_meta_read({file}, 'space.eidos.test'), '$.unknown.nested');
SELECT json_type(fs_meta_read({file}, 'space.eidos.test'), '$.nullable');
SELECT json_type(fs_meta_read({file}, 'space.eidos.test'), '$.old') IS NULL;
SELECT fs_meta_read({file}, 'space.eidos.other');
"#
    );
    assert_eq!(
        run_sqlite(temp.path(), Path::new(":memory:"), &sql),
        "{}\n4\n4\n5\n1\nnull\n1\n{}"
    );
    assert_eq!(
        fs::read_to_string(temp.path().join("note.txt")).unwrap(),
        "content"
    );
}

#[test]
fn namespace_properties_support_directories_and_the_scan_root() {
    let temp = tempdir().unwrap();
    let directory = temp.path().join("project.v1");
    fs::create_dir(&directory).unwrap();
    fs::write(directory.join("note.txt"), "unchanged").unwrap();
    let directory_sql = sql_string(&directory.to_string_lossy());
    let root = sql_string(&temp.path().to_string_lossy());
    let nested_file = sql_string(&directory.join("note.txt").to_string_lossy());
    let sql = format!(
        r#"
SELECT fs_meta_read({directory_sql}, 'space.eidos.test');
SELECT json_extract(fs_meta_patch({directory_sql}, 'space.eidos.test', '{{"rating":4,"unknown":{{"nested":true}},"obsolete":1}}', '[]'), '$.rating');
SELECT json_extract(fs_meta_patch({directory_sql}, 'space.eidos.test', '{{"nullable":null}}', '["obsolete"]'), '$.rating');
SELECT json_extract(fs_meta_read({directory_sql}, 'space.eidos.test'), '$.unknown.nested');
SELECT json_type(fs_meta_read({directory_sql}, 'space.eidos.test'), '$.nullable');
SELECT json_type(fs_meta_read({directory_sql}, 'space.eidos.test'), '$.obsolete') IS NULL;
SELECT fs_meta_read({directory_sql}, 'space.eidos.other');
SELECT fs_meta_read({nested_file}, 'space.eidos.test');
SELECT json_extract(fs_meta_patch({root}, 'space.eidos.test', '{{"owner":"root"}}', '[]'), '$.owner');
SELECT json_extract(fs_meta_read({root}, 'space.eidos.test'), '$.owner');
CREATE VIRTUAL TABLE entries USING fs_meta(root={root}, namespace='space.eidos.test', fields='rating INTEGER');
SELECT _id, rating, is_dir FROM entries WHERE _id='project.v1';
SELECT count(*) FROM entries WHERE _id='';
SELECT count(*) FROM entries WHERE _id='.';
SELECT instr(fs_meta_capabilities(), 'directory-properties') > 0;
"#
    );
    assert_eq!(
        run_sqlite(temp.path(), Path::new(":memory:"), &sql),
        "{}\n4\n4\n1\nnull\n1\n{}\n{}\nroot\nroot\nproject.v1|4|1\n0\n0\n1"
    );
    assert_eq!(
        fs::read_to_string(directory.join("note.txt")).unwrap(),
        "unchanged"
    );
}

#[test]
fn directories_are_queryable_and_metadata_changes_roll_back() {
    let temp = tempdir().unwrap();
    let directory = temp.path().join("project.v1");
    let nested = directory.join("nested");
    fs::create_dir_all(&nested).unwrap();
    fs::write(directory.join("note.txt"), "unchanged").unwrap();
    fs::write(nested.join("proof.pdf"), "proof").unwrap();
    fs::create_dir(temp.path().join(".hidden")).unwrap();
    fs::create_dir(temp.path().join("target")).unwrap();
    fs::write(temp.path().join("target/ignored.txt"), "ignored").unwrap();
    let directory_size = directory.metadata().unwrap().len();
    let directory_sql = sql_string(&directory.to_string_lossy());
    let root = sql_string(&temp.path().to_string_lossy());
    let sql = format!(
        r#"
CREATE VIRTUAL TABLE entries USING fs_meta(root={root}, namespace='space.eidos.test', fields='rating INTEGER, status TEXT');
SELECT id, name, extension, is_dir FROM entries ORDER BY id;
SELECT count(*) FROM entries WHERE is_dir=0;
SELECT count(*) FROM entries WHERE is_dir=1;
SELECT id FROM entries WHERE extension='pdf';
SELECT id, size={directory_size}, file IS NULL, mimetype IS NULL, mime_type IS NULL FROM entries WHERE id='project.v1';
SELECT _id, is_dir FROM entries WHERE path='project.v1/nested';
SELECT count(*) FROM entries WHERE id='project.v1' AND is_dir=0;
SELECT json_extract(fs_meta_patch({directory_sql}, 'space.eidos.test', '{{"rating":1,"unknown":{{"nested":true}},"retired":7}}', '[]'), '$.rating');
UPDATE entries SET rating=2, status='Active' WHERE _id='project.v1';
SELECT rating, status FROM entries WHERE id='project.v1';
SELECT json_extract(fs_meta_read({directory_sql}, 'space.eidos.test'), '$.unknown.nested');
BEGIN;
UPDATE entries SET rating=9 WHERE path='project.v1';
ROLLBACK;
SELECT rating FROM entries WHERE _id='project.v1';
BEGIN;
UPDATE entries SET rating=4 WHERE id='project.v1';
SAVEPOINT inner;
UPDATE entries SET rating=5 WHERE id='project.v1';
DELETE FROM entries WHERE id='project.v1';
ROLLBACK TO inner;
RELEASE inner;
COMMIT;
SELECT rating, status FROM entries WHERE _id='project.v1';
BEGIN;
UPDATE entries SET __fs_meta_remove_key='retired' WHERE id='project.v1';
SELECT json_type(fs_meta_read({directory_sql}, 'space.eidos.test'), '$.retired') IS NULL;
ROLLBACK;
SELECT json_extract(fs_meta_read({directory_sql}, 'space.eidos.test'), '$.retired');
UPDATE entries SET __fs_meta_remove_key='retired' WHERE id='project.v1';
UPDATE entries SET rating=NULL WHERE id='project.v1';
SELECT json_type(fs_meta_read({directory_sql}, 'space.eidos.test'), '$.rating') IS NULL;
SELECT json_type(fs_meta_read({directory_sql}, 'space.eidos.test'), '$.retired') IS NULL;
BEGIN;
DELETE FROM entries WHERE id='project.v1';
SELECT fs_meta_read({directory_sql}, 'space.eidos.test');
ROLLBACK;
SELECT status FROM entries WHERE id='project.v1';
SELECT json_extract(fs_meta_read({directory_sql}, 'space.eidos.test'), '$.unknown.nested');
DELETE FROM entries WHERE id='project.v1';
SELECT fs_meta_read({directory_sql}, 'space.eidos.test');
SELECT count(*) FROM entries WHERE id='project.v1';
"#
    );
    assert_eq!(
        run_sqlite(temp.path(), Path::new(":memory:"), &sql),
        "project.v1|project.v1||1\nproject.v1/nested|nested||1\nproject.v1/nested/proof.pdf|proof.pdf|pdf|0\nproject.v1/note.txt|note.txt|txt|0\n2\n2\nproject.v1/nested/proof.pdf\nproject.v1|1|1|1|1\nproject.v1/nested|1\n0\n1\n2|Active\n1\n2\n4|Active\n1\n7\n1\n1\n{}\nActive\n1\n{}\n1"
    );
    assert!(directory.is_dir());
    assert!(nested.is_dir());
    assert_eq!(
        fs::read_to_string(directory.join("note.txt")).unwrap(),
        "unchanged"
    );
    assert_eq!(
        fs::read_to_string(nested.join("proof.pdf")).unwrap(),
        "proof"
    );
}

#[test]
fn physical_deletion_removes_only_empty_directories() {
    let temp = tempdir().unwrap();
    let empty = temp.path().join("empty");
    let populated = temp.path().join("populated");
    fs::create_dir(&empty).unwrap();
    fs::create_dir(&populated).unwrap();
    fs::write(populated.join("keep.txt"), "keep").unwrap();
    let root = sql_string(&temp.path().to_string_lossy());
    let empty_sql = sql_string(&empty.to_string_lossy());
    let populated_sql = sql_string(&populated.to_string_lossy());
    let create = format!(
        "CREATE VIRTUAL TABLE entries USING fs_meta(root={root}, namespace='space.eidos.test', fields='rating INTEGER', on_delete='delete_file');"
    );
    assert_eq!(
        run_sqlite(
            temp.path(),
            Path::new(":memory:"),
            &format!(
                "{create} SELECT id, is_dir FROM entries WHERE is_dir=1 ORDER BY id; UPDATE entries SET rating=3 WHERE id='populated'; UPDATE entries SET rating=2 WHERE id='empty'; SELECT json_extract(fs_meta_read({empty_sql}, 'space.eidos.test'), '$.rating');"
            )
        ),
        "empty|1\npopulated|1\n2"
    );
    assert_sqlite_error(
        temp.path(),
        &format!("{create} DELETE FROM entries WHERE id='populated';"),
    );
    assert!(populated.is_dir());
    assert_eq!(
        fs::read_to_string(populated.join("keep.txt")).unwrap(),
        "keep"
    );
    assert_eq!(
        run_sqlite(
            temp.path(),
            Path::new(":memory:"),
            &format!(
                "{create} SELECT json_extract(fs_meta_read({populated_sql}, 'space.eidos.test'), '$.rating'); BEGIN; DELETE FROM entries WHERE id='empty'; ROLLBACK; SELECT count(*) FROM entries WHERE id='empty';"
            )
        ),
        "3\n0"
    );
    assert!(!empty.exists());
    assert_eq!(
        fs::read_to_string(populated.join("keep.txt")).unwrap(),
        "keep"
    );
}

#[test]
fn directory_links_are_not_scanned_or_used_for_properties() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("root");
    let outside = temp.path().join("outside");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("keep.txt"), "keep").unwrap();
    let link = root.join("linked");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, &link).unwrap();
    #[cfg(windows)]
    {
        // Junctions exercise reparse-point handling without symlink privileges.
        let output = Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(&outside)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "Cannot create test junction: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let root_sql = sql_string(&root.to_string_lossy());
    let link_sql = sql_string(&link.to_string_lossy());
    assert_eq!(
        run_sqlite(
            temp.path(),
            Path::new(":memory:"),
            &format!(
                "CREATE VIRTUAL TABLE entries USING fs_meta(root={root_sql}, fields='rating INTEGER'); SELECT count(*) FROM entries; SELECT count(*) FROM entries WHERE id='linked'; SELECT count(*) FROM entries WHERE path='linked/keep.txt'; UPDATE entries SET rating=9 WHERE id='linked'; DELETE FROM entries WHERE id='linked';"
            )
        ),
        "0\n0\n0"
    );
    assert_sqlite_error(
        temp.path(),
        &format!("SELECT fs_meta_read({link_sql}, 'space.eidos.test');"),
    );
    assert_sqlite_error(
        temp.path(),
        &format!("SELECT fs_meta_patch({link_sql}, 'space.eidos.test', '{{\"rating\":9}}', '[]');"),
    );
    assert!(link.exists());
    assert_eq!(
        fs::read_to_string(outside.join("keep.txt")).unwrap(),
        "keep"
    );
    let outside_sql = sql_string(&outside.to_string_lossy());
    assert_eq!(
        run_sqlite(
            temp.path(),
            Path::new(":memory:"),
            &format!("SELECT fs_meta_read({outside_sql}, 'space.eidos.test');")
        ),
        "{}"
    );
}

#[test]
fn test_sqlite_fs_meta_e2e() {
    let temp = tempdir().unwrap();
    let root = temp.path();

    // Create dummy files
    fs::write(root.join("doc1.pdf"), b"sample pdf content").unwrap();
    fs::create_dir_all(root.join("sub")).unwrap();
    fs::write(root.join("sub/image.png"), b"sample png content").unwrap();

    let sql = format!(
        r#"
CREATE VIRTUAL TABLE files USING fs_meta(
    root = {root},
    fields = 'tags TEXT, rating INTEGER, status TEXT'
);

-- Test SELECT
SELECT id, name, extension, mimetype, mime_type, tags, file FROM files ORDER BY id;

-- Test filtering by mimetype
SELECT id, mimetype FROM files WHERE mimetype LIKE 'image/%';

-- Test UPDATE
UPDATE files SET tags = '["review", "p0"]', rating = 5, status = 'Done' WHERE id = 'doc1.pdf';

-- Test SELECT after UPDATE
SELECT id, mimetype, tags, rating, status, file FROM files WHERE id = 'doc1.pdf';

-- Test DELETE (clearing metadata)
DELETE FROM files WHERE id = 'doc1.pdf';
SELECT id, tags, rating FROM files WHERE id = 'doc1.pdf';
"#,
        root = sql_string(&root.to_string_lossy())
    );

    let stdout = run_sqlite(root, Path::new(":memory:"), &sql);

    assert!(stdout.contains("doc1.pdf|doc1.pdf|pdf|application/pdf|application/pdf|"));
    assert!(stdout.contains("sub/image.png|image.png|png|image/png|image/png|"));
    assert!(stdout.contains("sub/image.png|image/png"));
    assert!(stdout.contains("doc1.pdf|application/pdf|[\"review\",\"p0\"]|5|Done"));
    assert!(stdout.contains(
        r#""mediaType":"application/pdf","name":"doc1.pdf","size":"18","uri":"doc1.pdf""#
    ));
    assert!(stdout.contains(
        r#""mediaType":"image/png","name":"image.png","size":"18","uri":"sub/image.png""#
    ));
}
