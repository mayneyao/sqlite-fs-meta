use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use tempfile::tempdir;

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

fn run_sqlite(cwd: &Path, database: &Path, sql: &str) -> String {
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
        sql_string(&extension_library().to_string_lossy())
    )
    .expect("Failed to send SQL to SQLite CLI");
    let output = child
        .wait_with_output()
        .expect("Failed to wait for SQLite CLI");
    assert!(
        output.status.success(),
        "SQLite CLI {executable:?} failed (extension loading is required): {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_string()
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
