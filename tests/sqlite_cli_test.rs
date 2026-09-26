use std::fs;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn relative_roots_follow_database_moves_and_decode_quoted_paths() {
    assert!(Command::new("cargo")
        .args(["build", "--locked"])
        .status()
        .unwrap()
        .success());
    let library = std::env::current_dir()
        .unwrap()
        .join("target/debug/libfs_meta");
    let temp = tempdir().unwrap();
    let root = temp.path().join("O'Brien \"notes\"");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("proof.txt"), "proof").unwrap();
    let run = |db: &std::path::Path, sql: &str| {
        let output =
            Command::new(std::env::var("SQLITE3_BIN").unwrap_or_else(|_| "sqlite3".into()))
                .current_dir(temp.path())
                .arg("-bail")
                .arg("-cmd")
                .arg(format!(".load {}", library.display()))
                .arg(db)
                .arg(sql)
                .output()
                .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_string()
    };
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
}

#[test]
fn test_sqlite_fs_meta_e2e() {
    let dylib_path = std::env::current_dir()
        .unwrap()
        .join("target/debug/libfs_meta");

    let status = Command::new("cargo")
        .arg("build")
        .status()
        .expect("Failed to build cdylib for test");
    assert!(status.success());

    let temp = tempdir().unwrap();
    let root = temp.path();

    // Create dummy files
    fs::write(root.join("doc1.pdf"), b"sample pdf content").unwrap();
    fs::create_dir_all(root.join("sub")).unwrap();
    fs::write(root.join("sub/image.png"), b"sample png content").unwrap();

    let sql = format!(
        r#"
.load {dylib}
CREATE VIRTUAL TABLE files USING fs_meta(
    root = '{root}',
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
        dylib = dylib_path.display(),
        root = root.display()
    );

    let output =
        match Command::new(std::env::var("SQLITE3_BIN").unwrap_or_else(|_| "sqlite3".into()))
            .arg(":memory:")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
        {
            Ok(mut child) => {
                use std::io::Write;
                if let Some(mut stdin) = child.stdin.take() {
                    let _ = stdin.write_all(sql.as_bytes());
                }
                match child.wait_with_output() {
                    Ok(out) => out,
                    Err(err) => {
                        eprintln!("Failed to wait for sqlite3: {}, skipping", err);
                        return;
                    }
                }
            }
            Err(err) => {
                eprintln!("sqlite3 CLI not available ({}), skipping", err);
                return;
            }
        };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if stderr.contains("unknown command or invalid arguments") || stderr.contains("not authorized")
    {
        eprintln!(
            "sqlite3 does not support extension loading in this environment, skipping test: {}",
            stderr
        );
        return;
    }

    assert!(
        output.status.success(),
        "sqlite3 failed with stderr: {}",
        stderr
    );

    println!("STDOUT:\n{}", stdout);

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
