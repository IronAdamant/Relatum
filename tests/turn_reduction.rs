//! Fewer agent turns: relatum_new with supersedes, relatum_new with notes
//! (all or none), and relatum_ls with include_bodies; and the CLI flags
//! that match them.

use relatum::json::{self, Value};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;

fn scratch(tag: &str) -> PathBuf {
    static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "relatum-turns-{tag}-{}-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(dir.join(".swarmnotes")).unwrap();
    dir
}

/// One stdio session in `cwd`; the result of each tools/call.
fn session(cwd: &Path, calls: &[(&str, String)]) -> Vec<Value> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_relatum-mcp"))
        .current_dir(cwd)
        .env_remove("SWARMNOTES_DIR")
        .env_remove("RELATUM_TOOLS")
        .env_remove("RELATUM_DISABLE")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn relatum-mcp");
    let mut stdin = child.stdin.take().unwrap();
    let mut reader = BufReader::new(child.stdout.take().unwrap());
    let mut send = |line: String| -> Value {
        writeln!(stdin, "{line}").unwrap();
        stdin.flush().unwrap();
        let mut reply = String::new();
        reader.read_line(&mut reply).unwrap();
        json::parse(reply.trim_end()).unwrap()
    };
    send(r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}"#.to_string());
    let results = calls
        .iter()
        .enumerate()
        .map(|(i, (name, args))| {
            let reply = send(format!(
                r#"{{"jsonrpc":"2.0","id":{},"method":"tools/call","params":{{"name":"{name}","arguments":{args}}}}}"#,
                i + 1
            ));
            reply.get("result").cloned().expect("result")
        })
        .collect();
    drop(stdin);
    child.wait().unwrap();
    results
}

fn call(cwd: &Path, name: &str, args: &str) -> Value {
    session(cwd, &[(name, args.to_string())]).remove(0)
}

fn text(result: &Value) -> String {
    result.get("content").and_then(Value::as_array).unwrap()[0]
        .get("text")
        .and_then(Value::as_str)
        .unwrap()
        .to_string()
}

fn is_error(result: &Value) -> bool {
    result.get("isError").and_then(Value::as_bool).unwrap()
}

fn ok(result: Value) -> Value {
    assert!(!is_error(&result), "{}", text(&result));
    json::parse(&text(&result)).unwrap()
}

/// Every file under the store, as (relative path, bytes), sorted.
fn snapshot(dir: &Path) -> Vec<(String, Vec<u8>)> {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let rel = path.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/");
            if path.is_dir() {
                out.push((format!("{rel}/"), Vec::new()));
                walk(base, &path, out);
            } else {
                out.push((rel, fs::read(&path).unwrap()));
            }
        }
    }
    let base = dir.join(".swarmnotes");
    let mut out = Vec::new();
    walk(&base, &base, &mut out);
    out.sort();
    out
}

fn new_args(id: &str, summary: &str, extra: &str) -> String {
    format!(r#"{{"id":"{id}","kind":"context","summary":"{summary}","body":"body of {id}","author":"a"{extra}}}"#)
}

fn status_of(dir: &Path, id: &str) -> String {
    let text = fs::read_to_string(dir.join(".swarmnotes").join(format!("{id}.md"))).unwrap();
    let note = relatum::meta::parse(&text);
    format!(
        "{}{}",
        note.get_str("status").unwrap_or("active"),
        note.get_str("superseded_by").map(|next| format!(" -> {next}")).unwrap_or_default()
    )
}

#[test]
fn new_with_supersedes_writes_and_marks_in_one_call() {
    let dir = scratch("supersedes");
    ok(call(&dir, "relatum_new", &new_args("m/api", "v1", "")));
    let result = ok(call(&dir, "relatum_new", &new_args("m/api-v2", "v2", r#","supersedes":"m/api""#)));
    assert_eq!(json::write_compact(&result), r#"{"id":"m/api-v2","superseded":"m/api"}"#);
    assert_eq!(status_of(&dir, "m/api"), "superseded -> m/api-v2");
    assert_eq!(status_of(&dir, "m/api-v2"), "active");
    let got = ok(call(&dir, "relatum_get", r#"{"ids":["m/api"]}"#));
    assert_eq!(got.get("notes").unwrap().as_array().unwrap()[0].get("id").unwrap().as_str(), Some("m/api-v2"));

    // A superseded, missing, or self target writes nothing.
    let before = snapshot(&dir);
    for (id, old, needle) in [
        ("m/api-v3", "m/api", "supersedes: m/api is already superseded by m/api-v2"),
        ("m/api-v3", "m/nope", "supersedes: no such note: m/nope"),
        ("m/api-v3", "Bad Id", "supersedes: invalid note id"),
        ("m/api-v2", "m/api-v2", "note already exists: m/api-v2"),
    ] {
        let result = call(&dir, "relatum_new", &new_args(id, "v3", &format!(r#","supersedes":"{old}""#)));
        assert!(is_error(&result));
        assert!(text(&result).contains(needle), "{}", text(&result));
    }
    assert_eq!(snapshot(&dir), before);
    // null and "" mean no supersede, as before.
    ok(call(&dir, "relatum_new", &new_args("m/x", "x", r#","supersedes":null"#)));
    let plain = ok(call(&dir, "relatum_new", &new_args("m/y", "y", r#","supersedes":"""#)));
    assert_eq!(json::write_compact(&plain), r#"{"id":"m/y"}"#);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn batch_writes_four_notes_and_supersedes_three() {
    let dir = scratch("batch");
    for id in ["m/auth-api", "m/store-api", "m/net-api"] {
        ok(call(&dir, "relatum_new", &new_args(id, "old", "")));
    }
    let notes = format!(
        "[{},{},{},{}]",
        new_args("m/auth-api-v2", "auth", r#","supersedes":"m/auth-api","tags":["api"]"#),
        new_args("m/store-api-v2", "store", r#","supersedes":"m/store-api""#),
        new_args("m/net-api-v2", "net", r#","supersedes":"m/net-api""#),
        new_args("m/handoff", "done", ""),
    );
    let result = ok(call(&dir, "relatum_new", &format!(r#"{{"notes":{notes}}}"#)));
    assert_eq!(
        json::write_compact(&result),
        r#"{"ids":["m/auth-api-v2","m/store-api-v2","m/net-api-v2","m/handoff"],"superseded":[{"old":"m/auth-api","new":"m/auth-api-v2"},{"old":"m/store-api","new":"m/store-api-v2"},{"old":"m/net-api","new":"m/net-api-v2"}]}"#
    );
    for (old, new) in [("m/auth-api", "m/auth-api-v2"), ("m/store-api", "m/store-api-v2"), ("m/net-api", "m/net-api-v2")] {
        assert_eq!(status_of(&dir, old), format!("superseded -> {new}"));
    }
    let listed = ok(call(&dir, "relatum_ls", r#"{"scope":"m"}"#));
    assert_eq!(listed.get("notes").unwrap().as_array().unwrap().len(), 4);
    let check = ok(call(&dir, "relatum_check", "{}"));
    assert_eq!(check.get("ok").and_then(Value::as_bool), Some(true), "{}", json::write_compact(&check));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_bad_batch_item_writes_nothing() {
    let dir = scratch("atomic");
    ok(call(&dir, "relatum_new", &new_args("m/api", "v1", "")));
    ok(call(&dir, "relatum_new", &new_args("m/taken", "t", "")));
    let before = snapshot(&dir);
    let good = new_args("fresh/one", "one", r#","supersedes":"m/api""#);
    let cases = [
        // Note errors, found before any write.
        (format!("[{good},{}]", new_args("fresh/two", "two", r#","supersedes":"m/missing""#)), "notes[1]: supersedes: no such note: m/missing"),
        (format!("[{good},{}]", new_args("m/taken", "two", "")), "notes[1]: note already exists: m/taken"),
        (format!("[{good},{}]", new_args("fresh/one", "dup", "")), "notes[1]: fresh/one appears twice in this call"),
        (format!("[{good},{}]", new_args("fresh/two", "again", r#","supersedes":"m/api""#)), "notes[1]: supersedes: m/api is superseded by another note in this call"),
        (format!(r#"[{good},{{"id":"fresh/two","kind":"nope","summary":"s","body":"b"}}]"#), "notes[1]: kind must be one of"),
        (format!(r#"[{good},{{"id":"fresh/two","kind":"context","summary":"{}","body":"b"}}]"#, "x".repeat(201)), "notes[1]: summary is 201 characters"),
        (format!(r#"[{good},{{"id":"Fresh","kind":"context","summary":"s","body":"b"}}]"#), "notes[1]: invalid note id"),
        // Shape errors, found before the store is touched.
        (format!(r#"[{good},{{"id":"fresh/two","kind":"context","summary":"s"}}]"#), "notes[1] is missing body"),
        (format!(r#"[{good},{{"id":"fresh/two","kind":"context","summary":"s","body":"b","titel":"x"}}]"#), "notes[1] has unknown field titel"),
        (format!(r#"[{good},{{"id":"fresh/two","kind":"context","summary":"s","body":3}}]"#), "notes[1].body must be a string, got 3 (number)"),
        (format!(r#"[{good},"fresh/two"]"#), "notes[1] must be an object"),
        ("[]".to_string(), "notes is empty"),
    ];
    for (notes, needle) in cases {
        let result = call(&dir, "relatum_new", &format!(r#"{{"notes":{notes}}}"#));
        assert!(is_error(&result), "{notes}");
        assert!(text(&result).contains(needle), "{needle}\n{}", text(&result));
        assert_eq!(snapshot(&dir), before, "{needle}");
    }
    let already = call(&dir, "relatum_new", &format!(r#"{{"notes":[{good},{}]}}"#, new_args("m/taken", "two", "")));
    assert!(text(&already).ends_with("Nothing was written."), "{}", text(&already));
    // One note's fields and notes together are refused; so is a non-list.
    let mixed = call(&dir, "relatum_new", &format!(r#"{{"id":"x","notes":[{good}]}}"#));
    assert!(text(&mixed).contains("invalid arguments: id is set beside notes"), "{}", text(&mixed));
    let wrong = call(&dir, "relatum_new", r#"{"notes":"fresh/one"}"#);
    assert!(text(&wrong).contains("invalid arguments: notes must be a list of note objects"), "{}", text(&wrong));
    assert_eq!(snapshot(&dir), before);
    let _ = fs::remove_dir_all(&dir);
}

/// A write that fails after the checks passed (here, a directory the
/// process may not write) undoes the notes already written and restores
/// the notes already marked, byte for byte.
#[cfg(unix)]
#[test]
fn a_failed_write_rolls_the_whole_batch_back() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch("rollback");
    ok(call(&dir, "relatum_new", &new_args("open/old", "o", "")));
    ok(call(&dir, "relatum_new", &new_args("locked/old", "l", "")));
    let locked = dir.join(".swarmnotes/locked");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).unwrap();
    if fs::write(locked.join("probe"), b"x").is_ok() {
        // Running as root: permissions do not stop the write.
        let _ = fs::remove_file(locked.join("probe"));
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        let _ = fs::remove_dir_all(&dir);
        return;
    }
    let before = snapshot(&dir);
    let notes = format!(
        "[{},{},{}]",
        new_args("fresh/deep/one", "one", ""),
        new_args("fresh/two", "two", r#","supersedes":"open/old""#),
        new_args("fresh/three", "three", r#","supersedes":"locked/old""#),
    );
    let result = call(&dir, "relatum_new", &format!(r#"{{"notes":{notes}}}"#));
    assert!(is_error(&result));
    assert!(text(&result).contains("notes[2]: could not supersede locked/old"), "{}", text(&result));
    assert_eq!(snapshot(&dir), before);
    assert!(!dir.join(".swarmnotes/fresh").exists());
    // A note that cannot be created rolls back the ones before it.
    let notes = format!("[{},{}]", new_args("fresh/one", "one", r#","supersedes":"open/old""#), new_args("locked/new", "n", ""));
    let result = call(&dir, "relatum_new", &format!(r#"{{"notes":{notes}}}"#));
    assert!(is_error(&result));
    assert!(text(&result).contains("notes[1]: "), "{}", text(&result));
    assert_eq!(snapshot(&dir), before);
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn ls_include_bodies_respects_filters_and_caps() {
    let dir = scratch("bodies");
    ok(call(&dir, "relatum_new", &new_args("m/old", "old", "")));
    ok(call(&dir, "relatum_new", &new_args("m/new", "new", r#","supersedes":"m/old","tags":["api"]"#)));
    ok(call(&dir, "relatum_new", r#"{"id":"m/finding","kind":"finding","summary":"f","body":"line one\nline two\n"}"#));
    let plain = ok(call(&dir, "relatum_ls", r#"{"scope":"m"}"#));
    assert!(!json::write_compact(&plain).contains("body"));
    let listed = ok(call(&dir, "relatum_ls", r#"{"scope":"m","include_bodies":true}"#));
    assert_eq!(
        json::write_compact(&listed),
        r#"{"notes":[{"id":"m/finding","kind":"finding","summary":"f","status":"active","body":"line one\nline two"},{"id":"m/new","kind":"context","summary":"new","status":"active","body":"body of m/new"}]}"#
    );
    let superseded = ok(call(&dir, "relatum_ls", r#"{"scope":"m","include_bodies":true,"include_superseded":true,"kind":"context"}"#));
    assert_eq!(
        json::write_compact(&superseded),
        r#"{"notes":[{"id":"m/new","kind":"context","summary":"new","status":"active","body":"body of m/new"},{"id":"m/old","kind":"context","summary":"old","status":"superseded","superseded_by":"m/new","body":"body of m/old"}]}"#
    );
    let tagged = ok(call(&dir, "relatum_ls", r#"{"scope":"m","include_bodies":true,"tag":"api","limit":1}"#));
    assert_eq!(tagged.get("notes").unwrap().as_array().unwrap().len(), 1);
    let bad = call(&dir, "relatum_ls", r#"{"include_bodies":"yes"}"#);
    assert!(text(&bad).contains("invalid arguments: include_bodies must be true or false"), "{}", text(&bad));

    // One body over the per-note cap is cut; past the total, bodies are left out.
    let big = "w".repeat(relatum::ops::LS_BODY_MAX + 10);
    let calls: Vec<(&str, String)> = (0..12)
        .map(|i| ("relatum_new", format!(r#"{{"id":"big/n{i:02}","kind":"context","summary":"s","body":"{big}"}}"#)))
        .collect();
    for result in session(&dir, &calls) {
        ok(result);
    }
    let listed = ok(call(&dir, "relatum_ls", r#"{"scope":"big","include_bodies":true}"#));
    let notes = listed.get("notes").unwrap().as_array().unwrap();
    assert_eq!(notes.len(), 12);
    let full = relatum::ops::LS_BODIES_TOTAL / relatum::ops::LS_BODY_MAX;
    for (index, note) in notes.iter().enumerate() {
        assert_eq!(note.get("truncated").and_then(Value::as_bool), Some(true));
        let body = note.get("body").and_then(Value::as_str);
        if index < full {
            assert_eq!(body.map(str::len), Some(relatum::ops::LS_BODY_MAX));
        } else {
            assert_eq!(body, None, "note {index}");
        }
    }
    let _ = fs::remove_dir_all(&dir);
}

static CWD: Mutex<()> = Mutex::new(());

fn cli(dir: &Path, args: &[&str], stdin: &str) -> (i32, String, String) {
    let old = std::env::current_dir().unwrap();
    std::env::set_current_dir(dir).unwrap();
    std::env::remove_var("SWARMNOTES_DIR");
    let owned: Vec<String> = args.iter().map(|arg| (*arg).to_string()).collect();
    let out = relatum::cli::run(&owned, "relatum", stdin, stdin.is_empty());
    std::env::set_current_dir(old).unwrap();
    out
}

#[test]
fn cli_supersedes_batch_and_bodies() {
    let _guard = CWD.lock().unwrap_or_else(|err| err.into_inner());
    let dir = scratch("cli");
    let new = |id: &str, extra: &[&str]| {
        let mut args = vec!["new", id, "--kind", "context", "--summary", "s", "--author", "a", "--body", "b"];
        args.extend_from_slice(extra);
        cli(&dir, &args, "")
    };
    assert_eq!(new("m/api", &[]).0, 0);
    let (code, out, err) = new("m/api-v2", &["--supersedes", "m/api"]);
    assert_eq!((code, out.as_str(), err.as_str()), (0, "m/api-v2\nm/api -> m/api-v2\n", ""));
    let (code, _, err) = new("m/api-v3", &["--supersedes", "m/api"]);
    assert_eq!(code, 1);
    assert_eq!(err, "relatum: supersedes: m/api is already superseded by m/api-v2; supersede the live note instead\n");

    let batch = r#"[{"id":"m/api-v3","kind":"context","summary":"v3","body":"b","supersedes":"m/api-v2"},
                    {"id":"m/handoff","kind":"context","summary":"h","body":"b","tags":["handoff"]}]"#;
    let (code, out, err) = cli(&dir, &["new", "--batch", "-"], batch);
    assert_eq!((code, out.as_str(), err.as_str()), (0, "m/api-v3\nm/api-v2 -> m/api-v3\nm/handoff\n", ""));
    fs::write(dir.join("batch.json"), r#"[{"id":"f/one","kind":"finding","summary":"o","body":"b"},{"id":"m/handoff","kind":"finding","summary":"x","body":"b"}]"#).unwrap();
    let before = snapshot(&dir);
    let (code, _, err) = cli(&dir, &["new", "--batch", "batch.json"], "");
    assert_eq!(code, 1);
    assert_eq!(err, "relatum: notes[1]: note already exists: m/handoff (pick another id) (nothing was written)\n");
    assert_eq!(snapshot(&dir), before);
    let (code, _, err) = cli(&dir, &["new", "--batch", "-"], r#"{"id":"x"}"#);
    assert_eq!((code, err.as_str()), (1, "relatum: --batch: expected a JSON array of notes\n"));
    let (code, _, err) = cli(&dir, &["new", "m/x", "--kind", "context", "--batch", "-"], "[]");
    assert_eq!(code, 2);
    assert!(err.ends_with("relatum new: error: argument --batch: not allowed with id, --kind (put them in the batch file)\n"), "{err}");
    // Without --batch the argparse errors are unchanged.
    let (code, _, err) = cli(&dir, &["new"], "");
    assert_eq!(code, 2);
    assert!(err.ends_with("error: the following arguments are required: id, --kind, --summary\n"), "{err}");

    let (code, out, _) = cli(&dir, &["ls", "m", "--bodies"], "");
    assert_eq!(code, 0);
    assert_eq!(out, "## m/api-v3 (context) \u{2014} v3\nb\n\n## m/handoff (context) \u{2014} h\nb\n");
    let (_, out, _) = cli(&dir, &["ls", "m", "--bodies", "--all", "--limit", "1"], "");
    assert_eq!(out, "## m/api-v2 (context) \u{2014} s [-> m/api-v3]\nb\n... 3 more (raise --limit, narrow the scope, or use --since)\n");
    let long = "z".repeat(relatum::ops::LS_BODY_MAX + 1);
    assert_eq!(cli(&dir, &["new", "l/long", "--kind", "context", "--summary", "s", "--body", &long], "").0, 0);
    let (_, out, _) = cli(&dir, &["ls", "l", "--bodies"], "");
    assert!(out.ends_with(&format!("{}\n[truncated; `relatum get l/long` prints the whole note]\n", "z".repeat(relatum::ops::LS_BODY_MAX))));
    let _ = fs::remove_dir_all(&dir);
}
