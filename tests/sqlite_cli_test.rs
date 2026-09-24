use std::fs;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn test_sqlite_fs_meta_e2e() {
    let dylib_path = std::env::current_dir()
        .unwrap()
        .join("target/debug/libfs_meta");

    if !dylib_path.with_extension("dylib").exists() {
        let status = Command::new("cargo")
            .arg("build")
            .status()
            .expect("Failed to build cdylib for test");
        assert!(status.success());
    }

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
SELECT id, name, extension, tags FROM files ORDER BY id;

-- Test UPDATE
UPDATE files SET tags = '["review", "p0"]', rating = 5, status = 'Done' WHERE id = 'doc1.pdf';

-- Test SELECT after UPDATE
SELECT id, tags, rating, status FROM files WHERE id = 'doc1.pdf';

-- Test DELETE (clearing metadata)
DELETE FROM files WHERE id = 'doc1.pdf';
SELECT id, tags, rating FROM files WHERE id = 'doc1.pdf';
"#,
        dylib = dylib_path.display(),
        root = root.display()
    );

    let output = Command::new("sqlite3")
        .arg(":memory:")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child.stdin.as_mut().unwrap().write_all(sql.as_bytes())?;
            child.wait_with_output()
        })
        .expect("Failed to execute sqlite3 CLI");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "sqlite3 failed with stderr: {}",
        stderr
    );

    println!("STDOUT:\n{}", stdout);

    assert!(stdout.contains("doc1.pdf|doc1.pdf|pdf|"));
    assert!(stdout.contains("sub/image.png|image.png|png|"));
    assert!(stdout.contains("doc1.pdf|[\"review\",\"p0\"]|5|Done"));
}
