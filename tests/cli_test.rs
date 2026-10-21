use fs_meta::properties::{DEFAULT_NAMESPACE, MAX_ENVELOPE_BYTES};
use serde_json::Value;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use tempfile::tempdir;

fn xmeta(cwd: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_xmeta"));
    command
        .current_dir(cwd)
        .env("HOME", cwd.join(".test-home"))
        .env("XDG_CONFIG_HOME", cwd.join(".test-config"))
        .env("APPDATA", cwd.join(".test-config"))
        .env_remove("FS_META_NAMESPACE");
    command
}

fn run(cwd: &Path, args: &[&str]) -> Output {
    xmeta(cwd).args(args).output().expect("failed to run xmeta")
}

fn run_stdin(cwd: &Path, args: &[&str], input: &str) -> Output {
    let mut child = xmeta(cwd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn xmeta");
    child
        .stdin
        .take()
        .expect("missing stdin")
        .write_all(input.as_bytes())
        .expect("failed to write stdin");
    child.wait_with_output().expect("failed to wait for xmeta")
}

fn assert_ok(output: &Output) {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_failure(output: &Output, code: i32) {
    assert_eq!(
        output.status.code(),
        Some(code),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn stdout_json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).expect("stdout is not JSON")
}

fn stderr_json(output: &Output) -> Value {
    serde_json::from_slice(&output.stderr).expect("stderr is not JSON")
}

fn write_file(path: &Path, content: &str) {
    fs::write(path, content).expect("failed to write test file");
}

#[test]
fn set_and_get_roundtrip() {
    let temp = tempdir().unwrap();
    let cwd = temp.path();
    write_file(&cwd.join("note.txt"), "content");

    let output = run(
        cwd,
        &[
            "set",
            "note.txt",
            "status=Active",
            "rating:=5",
            r#"tags:=["review","draft"]"#,
        ],
    );
    assert_ok(&output);
    assert!(output.stdout.is_empty());

    let output = run(cwd, &["--json", "get", "note.txt"]);
    assert_ok(&output);
    assert_eq!(
        output.stdout.iter().filter(|byte| **byte == b'\n').count(),
        1
    );
    assert_eq!(
        stdout_json(&output),
        serde_json::json!({
            "status": "Active",
            "rating": 5,
            "tags": ["review", "draft"]
        })
    );

    let output = run(cwd, &["get", "note.txt", "status", "--text"]);
    assert_ok(&output);
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "Active\n");

    let raw = run(cwd, &["get", "note.txt", "--raw"]);
    assert_ok(&raw);
    assert_eq!(
        raw.stdout,
        fs_meta::meta::read_envelope(&cwd.join("note.txt"), DEFAULT_NAMESPACE)
            .unwrap()
            .unwrap()
    );
}

#[test]
fn json_values_and_null_are_distinct_from_removal() {
    let temp = tempdir().unwrap();
    let cwd = temp.path();
    write_file(&cwd.join("note.txt"), "content");

    assert_ok(&run(
        cwd,
        &[
            "set",
            "note.txt",
            "--json-values",
            "rating=5",
            "archived=false",
            "empty=null",
        ],
    ));

    let output = run(cwd, &["--json", "get", "note.txt"]);
    assert_eq!(
        stdout_json(&output),
        serde_json::json!({"rating": 5, "archived": false, "empty": null})
    );

    let output = run(cwd, &["get", "note.txt", "empty", "--text"]);
    assert_ok(&output);
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "null\n");

    assert_ok(&run(cwd, &["unset", "note.txt", "empty"]));
    let output = run(cwd, &["--json", "get", "note.txt"]);
    assert_eq!(
        stdout_json(&output),
        serde_json::json!({"rating": 5, "archived": false})
    );
}

#[test]
fn patch_merges_and_removes() {
    let temp = tempdir().unwrap();
    let cwd = temp.path();
    write_file(&cwd.join("note.txt"), "content");

    assert_ok(&run(cwd, &["set", "note.txt", "a=1", "b=2"]));
    let output = run(
        cwd,
        &[
            "patch", "note.txt", "--set", "c:=3", "--remove", "a", "--print",
        ],
    );
    assert_ok(&output);
    assert_eq!(stdout_json(&output), serde_json::json!({"b": "2", "c": 3}));
}

#[test]
fn unset_is_idempotent_unless_strict() {
    let temp = tempdir().unwrap();
    let cwd = temp.path();
    write_file(&cwd.join("note.txt"), "content");

    assert_ok(&run(cwd, &["set", "note.txt", "a=1"]));
    assert_ok(&run(cwd, &["unset", "note.txt", "missing"]));

    let output = run(cwd, &["--json", "unset", "note.txt", "missing", "--strict"]);
    assert_failure(&output, 3);
    assert_eq!(
        stderr_json(&output)["error"]["code"],
        Value::String("not_found".into())
    );
}

#[test]
fn missing_key_has_no_partial_output() {
    let temp = tempdir().unwrap();
    let cwd = temp.path();
    write_file(&cwd.join("note.txt"), "content");

    assert_ok(&run(cwd, &["set", "note.txt", "a=1"]));
    let output = run(cwd, &["--json", "get", "note.txt", "a", "missing"]);
    assert_failure(&output, 3);
    assert!(output.stdout.is_empty());
    let error = stderr_json(&output);
    assert_eq!(error["error"]["code"], Value::String("not_found".into()));
    assert_eq!(error["error"]["key"], Value::String("missing".into()));
}

#[test]
fn namespaces_are_isolated() {
    let temp = tempdir().unwrap();
    let cwd = temp.path();
    write_file(&cwd.join("note.txt"), "content");

    assert_ok(&run(cwd, &["set", "note.txt", "a=default"]));
    assert_ok(&run(
        cwd,
        &["-n", "space.eidos.other", "set", "note.txt", "a=other"],
    ));

    let default = run(cwd, &["get", "note.txt", "a", "--text"]);
    assert_ok(&default);
    assert_eq!(String::from_utf8(default.stdout).unwrap(), "default\n");

    let other = run(
        cwd,
        &["-n", "space.eidos.other", "get", "note.txt", "a", "--text"],
    );
    assert_ok(&other);
    assert_eq!(String::from_utf8(other.stdout).unwrap(), "other\n");

    assert_ok(&run(cwd, &["-n", "space.eidos.other", "clear", "note.txt"]));
    let output = run(cwd, &["get", "note.txt", "a", "--text"]);
    assert_ok(&output);
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "default\n");
}

#[test]
fn namespace_env_is_used_and_flag_wins() {
    let temp = tempdir().unwrap();
    let cwd = temp.path();
    write_file(&cwd.join("note.txt"), "content");

    let set = xmeta(cwd)
        .env("FS_META_NAMESPACE", "space.eidos.env")
        .args(["set", "note.txt", "a=env"])
        .output()
        .unwrap();
    assert_ok(&set);

    let get = xmeta(cwd)
        .env("FS_META_NAMESPACE", "space.eidos.env")
        .args(["get", "note.txt", "a", "--text"])
        .output()
        .unwrap();
    assert_ok(&get);
    assert_eq!(String::from_utf8(get.stdout).unwrap(), "env\n");

    let overridden = xmeta(cwd)
        .env("FS_META_NAMESPACE", "space.eidos.env")
        .args(["-n", DEFAULT_NAMESPACE, "get", "note.txt", "a"])
        .output()
        .unwrap();
    assert_failure(&overridden, 3);
}

#[test]
fn directory_properties_are_not_inherited() {
    let temp = tempdir().unwrap();
    let cwd = temp.path();
    fs::create_dir(cwd.join("folder")).unwrap();
    write_file(&cwd.join("folder/child.txt"), "content");

    assert_ok(&run(cwd, &["set", "folder", "status=Active"]));
    let folder = run(cwd, &["get", "folder", "status", "--text"]);
    assert_ok(&folder);
    assert_eq!(String::from_utf8(folder.stdout).unwrap(), "Active\n");

    let child = run(cwd, &["--json", "get", "folder/child.txt"]);
    assert_ok(&child);
    assert_eq!(stdout_json(&child), serde_json::json!({}));

    assert_ok(&run(cwd, &["clear", "folder"]));
    let cleared = run(cwd, &["--json", "get", "folder"]);
    assert_ok(&cleared);
    assert_eq!(stdout_json(&cleared), serde_json::json!({}));
}

#[test]
fn corrupt_envelopes_require_explicit_replacement() {
    let temp = tempdir().unwrap();
    let cwd = temp.path();
    let path = cwd.join("note.txt");
    write_file(&path, "content");
    fs_meta::meta::restore_envelope(&path, DEFAULT_NAMESPACE, Some(b"corrupt")).unwrap();

    assert_failure(&run(cwd, &["get", "note.txt"]), 4);
    let raw = run(cwd, &["get", "note.txt", "--raw"]);
    assert_ok(&raw);
    assert_eq!(raw.stdout, b"corrupt");

    assert_failure(&run(cwd, &["patch", "note.txt", "--set", "a:=1"]), 4);

    let replaced = run(cwd, &["set", "note.txt", "--replace", "a:=1", "--print"]);
    assert_ok(&replaced);
    assert_eq!(stdout_json(&replaced), serde_json::json!({"a": 1}));

    assert_ok(&run(cwd, &["clear", "note.txt"]));
    let empty = run(cwd, &["--json", "get", "note.txt"]);
    assert_ok(&empty);
    assert_eq!(stdout_json(&empty), serde_json::json!({}));
}

#[test]
fn dry_run_does_not_write() {
    let temp = tempdir().unwrap();
    let cwd = temp.path();
    write_file(&cwd.join("note.txt"), "content");

    assert_ok(&run(cwd, &["set", "note.txt", "rating:=5"]));
    let preview = run(
        cwd,
        &["patch", "note.txt", "--set", "rating:=6", "--dry-run"],
    );
    assert_ok(&preview);
    assert_eq!(stdout_json(&preview), serde_json::json!({"rating": 6}));

    let actual = run(cwd, &["get", "note.txt", "rating", "--text"]);
    assert_ok(&actual);
    assert_eq!(String::from_utf8(actual.stdout).unwrap(), "5\n");
}

#[test]
fn patch_accepts_json_from_stdin_and_files() {
    let temp = tempdir().unwrap();
    let cwd = temp.path();
    write_file(&cwd.join("note.txt"), "content");
    write_file(
        &cwd.join("meta.json"),
        r#"{"owner":"team","priority":"P1"}"#,
    );

    let output = run_stdin(
        cwd,
        &["patch", "note.txt", "--from", "-", "--print"],
        r#"{"owner":"team","archived":false}"#,
    );
    assert_ok(&output);
    assert_eq!(
        stdout_json(&output),
        serde_json::json!({"owner": "team", "archived": false})
    );

    let output = run(
        cwd,
        &["patch", "note.txt", "--from", "meta.json", "--print"],
    );
    assert_ok(&output);
    assert_eq!(
        stdout_json(&output),
        serde_json::json!({
            "owner": "team",
            "archived": false,
            "priority": "P1"
        })
    );

    let invalid = run_stdin(cwd, &["patch", "note.txt", "--from", "-"], "[]");
    assert_failure(&invalid, 4);

    let missing = run(cwd, &["patch", "note.txt", "--from", "missing.json"]);
    assert_failure(&missing, 3);
}

#[test]
fn replace_rewrites_the_namespace() {
    let temp = tempdir().unwrap();
    let cwd = temp.path();
    write_file(&cwd.join("note.txt"), "content");

    assert_ok(&run(cwd, &["set", "note.txt", "a=1", "b=2"]));
    let output = run(cwd, &["set", "note.txt", "--replace", "c:=3", "--print"]);
    assert_ok(&output);
    assert_eq!(stdout_json(&output), serde_json::json!({"c": 3}));

    let values = run(cwd, &["--json", "get", "note.txt"]);
    assert_eq!(stdout_json(&values), serde_json::json!({"c": 3}));
}

#[test]
fn conflicting_set_and_remove_is_reported() {
    let temp = tempdir().unwrap();
    let cwd = temp.path();
    write_file(&cwd.join("note.txt"), "content");
    assert_ok(&run(cwd, &["set", "note.txt", "a=1"]));

    let output = run(
        cwd,
        &[
            "--json", "patch", "note.txt", "--set", "a:=2", "--remove", "a",
        ],
    );
    assert_failure(&output, 6);
    assert_eq!(
        stderr_json(&output)["error"]["code"],
        Value::String("conflict".into())
    );
}

#[test]
fn raw_and_text_option_validation() {
    let temp = tempdir().unwrap();
    let cwd = temp.path();
    write_file(&cwd.join("note.txt"), "content");
    assert_ok(&run(cwd, &["set", "note.txt", "a=1"]));

    assert_failure(&run(cwd, &["get", "note.txt", "a", "--raw"]), 2);
    assert_failure(&run(cwd, &["get", "note.txt", "--text"]), 2);
    assert_failure(&run(cwd, &["get", "note.txt", "a", "--text", "--json"]), 2);

    assert_ok(&run(cwd, &["set", "note.txt", "obj:={\"x\":1}"]));
    assert_failure(&run(cwd, &["get", "note.txt", "obj", "--text"]), 4);
}

#[test]
fn invalid_input_and_assignments_are_rejected() {
    let temp = tempdir().unwrap();
    let cwd = temp.path();
    write_file(&cwd.join("note.txt"), "content");

    assert_failure(&run(cwd, &["set", "note.txt", "missing-separator"]), 2);
    assert_failure(
        &run(cwd, &["set", "note.txt", "--json-values", "a=not-json"]),
        4,
    );
    assert_failure(&run(cwd, &["-n", "../escape", "get", "note.txt"]), 4);
    assert_failure(&run(cwd, &["set", "missing.txt", "a=1"]), 3);
    assert_failure(&run(cwd, &["clear", "missing.txt"]), 3);
}

#[test]
fn oversized_input_is_rejected() {
    let temp = tempdir().unwrap();
    let cwd = temp.path();
    write_file(&cwd.join("note.txt"), "content");

    let input = format!(r#"{{"note":"{}"}}"#, "x".repeat(MAX_ENVELOPE_BYTES + 1));
    let output = run_stdin(cwd, &["--json", "patch", "note.txt", "--from", "-"], &input);
    assert_failure(&output, 4);
    assert_eq!(
        stderr_json(&output)["error"]["code"],
        Value::String("invalid".into())
    );
}

#[test]
fn raw_absent_namespace_is_not_found() {
    let temp = tempdir().unwrap();
    let cwd = temp.path();
    write_file(&cwd.join("note.txt"), "content");

    let output = run(cwd, &["get", "note.txt", "--raw"]);
    assert_failure(&output, 3);
}

#[test]
fn paths_with_spaces_and_unicode() {
    let temp = tempdir().unwrap();
    let cwd = temp.path();
    write_file(&cwd.join("note file 你好.txt"), "content");

    assert_ok(&run(cwd, &["set", "note file 你好.txt", "status=Active"]));
    let output = run(cwd, &["get", "note file 你好.txt", "status", "--text"]);
    assert_ok(&output);
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "Active\n");
}

#[test]
fn help_and_version_succeed() {
    let temp = tempdir().unwrap();
    let cwd = temp.path();
    assert_ok(&run(cwd, &["--help"]));
    let version = run(cwd, &["--version"]);
    assert_ok(&version);
    assert!(String::from_utf8_lossy(&version.stdout).contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn sql_storage_interoperates_with_cli() {
    let temp = tempdir().unwrap();
    let cwd = temp.path();
    let path = cwd.join("note.txt");
    write_file(&path, "content");

    assert_ok(&run(cwd, &["set", "note.txt", "rating:=5"]));
    let stored =
        fs_meta::meta::read_metadata(&path, DEFAULT_NAMESPACE).expect("metadata read failed");
    assert_eq!(stored.get("rating"), Some(&serde_json::json!(5)));

    let mut metadata = std::collections::HashMap::new();
    metadata.insert("owner".to_owned(), serde_json::json!("team"));
    fs_meta::meta::write_metadata(&path, DEFAULT_NAMESPACE, &metadata).unwrap();

    let output = run(cwd, &["--json", "get", "note.txt"]);
    assert_ok(&output);
    assert_eq!(stdout_json(&output), serde_json::json!({"owner": "team"}));
}

#[cfg(unix)]
#[test]
fn unix_symlinks_and_hardlinks_are_rejected_for_writes() {
    let temp = tempdir().unwrap();
    let cwd = temp.path();
    let target = cwd.join("note.txt");
    write_file(&target, "content");
    let ns = DEFAULT_NAMESPACE;

    std::os::unix::fs::symlink(&target, cwd.join("note-link.txt")).unwrap();
    assert_failure(&run(cwd, &["get", "note-link.txt"]), 4);
    assert_failure(&run(cwd, &["set", "note-link.txt", "a=1"]), 4);

    std::fs::hard_link(&target, cwd.join("note-hard.txt")).unwrap();
    assert_ok(&run(cwd, &["get", "note.txt"]));
    assert_failure(&run(cwd, &["set", "note.txt", "a=1"]), 4);
    assert_failure(&run(cwd, &["clear", "note.txt"]), 4);
    assert_eq!(
        fs_meta::meta::read_envelope(&target, ns).unwrap(),
        None,
        "rejected writes must not create metadata"
    );
}

#[test]
fn config_set_updates_user_namespace() {
    let temp = tempdir().unwrap();
    let cwd = temp.path();
    write_file(&cwd.join("note.txt"), "content");

    assert_ok(&run(cwd, &["config", "set", "space.eidos.user"]));
    let get = run(cwd, &["config", "get"]);
    assert_ok(&get);
    assert_eq!(String::from_utf8(get.stdout).unwrap(), "space.eidos.user\n");

    let get_json = run(cwd, &["--json", "config", "get"]);
    assert_ok(&get_json);
    assert_eq!(stdout_json(&get_json)["namespace"], "space.eidos.user");

    assert_ok(&run(cwd, &["set", "note.txt", "a=1"]));
    assert_eq!(
        fs_meta::meta::read_envelope(&cwd.join("note.txt"), "space.eidos.user").unwrap(),
        Some(br#"{"a":"1"}"#.to_vec())
    );
    assert_eq!(
        fs_meta::meta::read_envelope(&cwd.join("note.txt"), DEFAULT_NAMESPACE).unwrap(),
        None
    );

    let report = run(cwd, &["--json", "config"]);
    assert_ok(&report);
    let report = stdout_json(&report);
    assert_eq!(report["namespace"], "space.eidos.user");
    assert_eq!(report["source"], "user");

    assert_ok(&run(cwd, &["config", "unset"]));
    assert!(!cwd.join(".test-config/xmeta/config.json").exists());
    let get = run(cwd, &["config", "get"]);
    assert_ok(&get);
    assert_eq!(
        String::from_utf8(get.stdout).unwrap(),
        format!("{DEFAULT_NAMESPACE}\n")
    );
}

#[test]
fn project_config_is_discovered_from_the_target_path() {
    let temp = tempdir().unwrap();
    let cwd = temp.path();
    let space = cwd.join("space");
    fs::create_dir_all(space.join("sub")).unwrap();
    write_file(
        &space.join(".xmeta.json"),
        r#"{"namespace":"space.eidos.project"}"#,
    );
    write_file(&space.join("sub/note.txt"), "content");

    let target = space.join("sub/note.txt").to_string_lossy().into_owned();
    assert_ok(&run(cwd, &["set", &target, "a=1"]));
    assert_eq!(
        fs_meta::meta::read_envelope(&space.join("sub/note.txt"), "space.eidos.project").unwrap(),
        Some(br#"{"a":"1"}"#.to_vec())
    );

    let get = run(&space, &["config", "get"]);
    assert_ok(&get);
    assert_eq!(
        String::from_utf8(get.stdout).unwrap(),
        "space.eidos.project\n"
    );

    let report = run(&space, &["--json", "config"]);
    assert_ok(&report);
    let report = stdout_json(&report);
    assert_eq!(report["source"], "project");
    assert_eq!(
        report["source_path"],
        fs::canonicalize(&space)
            .unwrap()
            .join(".xmeta.json")
            .to_string_lossy()
            .as_ref()
    );
}

#[test]
fn namespace_precedence_is_cli_then_env_then_project_then_user() {
    let temp = tempdir().unwrap();
    let cwd = temp.path();
    write_file(&cwd.join("note.txt"), "content");

    assert_ok(&run(cwd, &["config", "set", "space.eidos.user"]));
    write_file(
        &cwd.join(".xmeta.json"),
        r#"{"namespace":"space.eidos.project"}"#,
    );

    let project = run(cwd, &["config", "get"]);
    assert_ok(&project);
    assert_eq!(
        String::from_utf8(project.stdout).unwrap(),
        "space.eidos.project\n"
    );

    let env = xmeta(cwd)
        .env("FS_META_NAMESPACE", "space.eidos.env")
        .args(["config", "get"])
        .output()
        .unwrap();
    assert_ok(&env);
    assert_eq!(String::from_utf8(env.stdout).unwrap(), "space.eidos.env\n");

    let cli = run(cwd, &["-n", "space.eidos.cli", "config", "get"]);
    assert_ok(&cli);
    assert_eq!(String::from_utf8(cli.stdout).unwrap(), "space.eidos.cli\n");

    assert_ok(&run(cwd, &["set", "note.txt", "a=1"]));
    assert_eq!(
        fs_meta::meta::read_envelope(&cwd.join("note.txt"), "space.eidos.project").unwrap(),
        Some(br#"{"a":"1"}"#.to_vec())
    );
}

#[test]
fn explicit_config_disables_automatic_discovery() {
    let temp = tempdir().unwrap();
    let cwd = temp.path();
    write_file(&cwd.join("note.txt"), "content");
    write_file(
        &cwd.join(".xmeta.json"),
        r#"{"namespace":"space.eidos.project"}"#,
    );
    write_file(
        &cwd.join("explicit.json"),
        r#"{"namespace":"space.eidos.explicit"}"#,
    );

    let get = run(cwd, &["--config", "explicit.json", "config", "get"]);
    assert_ok(&get);
    assert_eq!(
        String::from_utf8(get.stdout).unwrap(),
        "space.eidos.explicit\n"
    );

    assert_ok(&run(
        cwd,
        &["--config", "explicit.json", "set", "note.txt", "a=1"],
    ));
    assert_eq!(
        fs_meta::meta::read_envelope(&cwd.join("note.txt"), "space.eidos.explicit").unwrap(),
        Some(br#"{"a":"1"}"#.to_vec())
    );

    assert_failure(&run(cwd, &["--config", "missing.json", "config", "get"]), 3);
}

#[test]
fn malformed_or_invalid_config_is_rejected() {
    let temp = tempdir().unwrap();
    let cwd = temp.path();
    write_file(&cwd.join("note.txt"), "content");

    write_file(&cwd.join(".xmeta.json"), "");
    let empty = run(cwd, &["config", "get"]);
    assert_ok(&empty);
    assert_eq!(
        String::from_utf8(empty.stdout).unwrap(),
        format!("{DEFAULT_NAMESPACE}\n")
    );

    write_file(&cwd.join(".xmeta.json"), "{");
    let output = run(cwd, &["get", "note.txt"]);
    assert_failure(&output, 4);
    assert!(String::from_utf8_lossy(&output.stderr).contains(".xmeta.json"));

    write_file(&cwd.join(".xmeta.json"), r#"{"namespace":"../escape"}"#);
    assert_failure(&run(cwd, &["get", "note.txt"]), 4);

    write_file(&cwd.join(".xmeta.json"), r#"{"namespace":5}"#);
    assert_failure(&run(cwd, &["get", "note.txt"]), 4);

    assert_failure(&run(cwd, &["config", "set", "../escape"]), 4);
}

#[test]
fn config_project_target_preserves_unknown_keys() {
    let temp = tempdir().unwrap();
    let cwd = temp.path();
    write_file(&cwd.join("note.txt"), "content");
    write_file(
        &cwd.join(".xmeta.json"),
        r#"{"namespace":"space.eidos.old","future":true}"#,
    );

    assert_ok(&run(
        cwd,
        &["config", "set", "--project", "space.eidos.new"],
    ));
    let config: Value =
        serde_json::from_slice(&fs::read(cwd.join(".xmeta.json")).unwrap()).unwrap();
    assert_eq!(config["namespace"], "space.eidos.new");
    assert_eq!(config["future"], true);

    let path = run(cwd, &["config", "path", "--project"]);
    assert_ok(&path);
    assert_eq!(
        String::from_utf8(path.stdout).unwrap(),
        format!(
            "{}\n",
            fs::canonicalize(cwd).unwrap().join(".xmeta.json").display()
        )
    );

    assert_ok(&run(cwd, &["config", "unset", "--project"]));
    let config: Value =
        serde_json::from_slice(&fs::read(cwd.join(".xmeta.json")).unwrap()).unwrap();
    assert!(config.get("namespace").is_none());
    assert_eq!(config["future"], true);
}

#[test]
fn config_path_and_user_target_are_stable() {
    let temp = tempdir().unwrap();
    let cwd = temp.path();
    write_file(&cwd.join("note.txt"), "content");

    let path = run(cwd, &["config", "path"]);
    assert_ok(&path);
    let expected_path = cwd.join(".test-config/xmeta/config.json");
    assert_eq!(
        String::from_utf8(path.stdout).unwrap(),
        format!("{}\n", expected_path.display())
    );

    let path_json = run(cwd, &["--json", "config", "path"]);
    assert_ok(&path_json);
    assert_eq!(
        stdout_json(&path_json)["path"],
        expected_path.to_string_lossy().as_ref()
    );

    assert_ok(&run(cwd, &["config", "set", "--project", "space.eidos.p"]));
    assert_eq!(
        fs_meta::meta::read_envelope(&cwd.join("note.txt"), "space.eidos.p").unwrap(),
        None
    );

    let project = run(cwd, &["config", "path", "--project"]);
    assert_ok(&project);
    assert_eq!(
        String::from_utf8(project.stdout).unwrap(),
        format!(
            "{}\n",
            fs::canonicalize(cwd).unwrap().join(".xmeta.json").display()
        )
    );

    assert_ok(&run(cwd, &["config", "unset", "--project"]));
    assert!(!cwd.join(".xmeta.json").exists());
}
