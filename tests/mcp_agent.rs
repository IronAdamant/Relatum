//! What an agent sees over MCP: lean descriptions, compact results, and
//! errors that name the next step.

use relatum::json::{self, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "relatum-agent-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Run one stdio session in `cwd` and return the result of each tools/call.
fn session(cwd: &Path, calls: &[(&str, &str)]) -> Vec<Value> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_relatum-mcp"))
        .current_dir(cwd)
        .env_remove("SWARMNOTES_DIR")
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

fn text(result: &Value) -> &str {
    result.get("content").and_then(Value::as_array).unwrap()[0]
        .get("text")
        .and_then(Value::as_str)
        .unwrap()
}

fn is_error(result: &Value) -> bool {
    result.get("isError").and_then(Value::as_bool).unwrap()
}

#[test]
fn tool_descriptions_carry_no_docstring_indentation() {
    let tools = relatum::mcp_schema::tools();
    for tool in tools.as_array().unwrap() {
        let description = tool.get("description").and_then(Value::as_str).unwrap();
        for line in description.lines() {
            assert_eq!(line, line.trim(), "padded line in {description:?}");
        }
        assert!(description.ends_with(".\n"), "{description:?}");
    }
}

#[test]
fn results_are_compact_json_matching_structured_content() {
    let dir = scratch("compact");
    let results = session(
        &dir,
        &[
            ("relatum_init", "{}"),
            ("relatum_ls", "{}"),
            ("relatum_get", r#"{"ids":["_shared/report-format"]}"#),
        ],
    );
    for result in &results {
        assert!(!is_error(result), "{}", text(result));
        let structured = result.get("structuredContent").and_then(|s| s.get("result")).unwrap();
        assert_eq!(text(result), json::write_compact(structured));
        assert!(!text(result).contains("\n  "), "{}", text(result));
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn errors_name_the_next_step() {
    let dir = scratch("errors");
    let results = session(
        &dir,
        &[
            ("relatum_ls", "{}"),
            ("relatum_init", "{}"),
            ("relatum_ls", r#"{"scope":"work/a"}"#),
            ("relatum_ls", r#"{"scope":"Not A Scope"}"#),
            ("relatum_new", r#"{"id":"Plan/Bad Id","kind":"context","summary":"s","body":"b"}"#),
            ("relatum_get", r#"{"ids":["Bad Id"]}"#),
            ("relatum_new", r#"{"id":"work/a/first","kind":"finding","summary":"s","body":"b"}"#),
            ("relatum_ls", r#"{"scope":"work/a"}"#),
        ],
    );
    let expect_error = |i: usize, needle: &str| {
        assert!(is_error(&results[i]), "call {i}: {}", text(&results[i]));
        assert!(text(&results[i]).contains(needle), "call {i}: {}", text(&results[i]));
    };
    expect_error(0, "Call relatum_init to create one there");
    assert!(text(&results[0]).contains("SWARMNOTES_DIR"));
    assert!(!is_error(&results[1]));
    expect_error(2, "no such scope: work/a. Nothing is written there yet; relatum_new with an id like work/a/<slug> creates it.");
    expect_error(3, "no such scope: Not A Scope. relatum_ls with no scope lists every note.");
    expect_error(4, "invalid note id: 'Plan/Bad Id'. Ids are lowercase segments joined by /, each matching _?[a-z0-9][a-z0-9-]*, e.g. net/retry-decision.");
    let got = results[5].get("structuredContent").and_then(|s| s.get("result")).unwrap();
    let error = got.get("errors").and_then(Value::as_array).unwrap()[0]
        .get("error")
        .and_then(Value::as_str)
        .unwrap();
    assert!(error.ends_with("e.g. net/retry-decision."), "{error}");
    assert!(!is_error(&results[6]), "{}", text(&results[6]));
    assert!(!is_error(&results[7]), "{}", text(&results[7]));
    assert!(text(&results[7]).contains("work/a/first"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn validation_errors_are_one_actionable_line() {
    let dir = scratch("validation");
    let long = "x".repeat(60);
    let results = session(
        &dir,
        &[
            ("relatum_init", "{}"),
            ("relatum_new", r#"{"id":"only"}"#),
            ("relatum_new", r#"{"id":"a","kind":"finding","summary":"s"}"#),
            ("relatum_new", r#"{"id":"a","kind":"finding","summary":5,"body":"b"}"#),
            ("relatum_new", r#"{"id":"a","kind":"finding","summary":"s","body":"b","tags":"net"}"#),
            ("relatum_new", r#"{"id":"a","kind":"finding","summary":"s","body":"b","tags":["net",3]}"#),
            ("relatum_get", r#"{"ids":"net/retry"}"#),
            ("relatum_get", "{}"),
            ("relatum_supersede", r#"{"old":"a"}"#),
            ("relatum_ls", r#"{"include_superseded":"yes"}"#),
            ("relatum_ls", &format!(r#"{{"scope":["{long}"]}}"#)),
            ("relatum_init", r#"{"path":null}"#),
        ],
    );
    let expected = [
        "missing required arguments kind, summary, body. relatum_new needs id, kind, summary, and body.",
        "missing required argument body. relatum_new needs id, kind, summary, and body.",
        "summary must be a string, got 5 (number). Call relatum_new again with summary like \"text\".",
        "tags must be a list of strings, got \"net\" (string). Call relatum_new again with tags like [\"net/retry\"].",
        "tags must be a list of strings, got [\"net\",3] (list). Call relatum_new again with tags like [\"net/retry\"].",
        "ids must be a list of strings, got \"net/retry\" (string). Call relatum_get again with ids like [\"net/retry\"].",
        "missing required argument ids. relatum_get needs ids.",
        "missing required argument new. relatum_supersede needs old and new.",
        "include_superseded must be true or false, got \"yes\" (string). Call relatum_ls again with include_superseded like true.",
        "scope must be a string, got a long list. Call relatum_ls again with scope like \"text\".",
        "path must be a string, got null. Call relatum_init again with path like \"text\".",
    ];
    assert!(!is_error(&results[0]));
    for (result, want) in results[1..].iter().zip(expected) {
        assert!(is_error(result), "{want}");
        let got = text(result);
        let tool = got.strip_prefix("Error executing tool ").unwrap().split(':').next().unwrap();
        assert_eq!(got, format!("Error executing tool {tool}: invalid arguments: {want}"));
        assert!(!got.contains('\n') && !got.contains("pydantic") && !got.contains("http"), "{got}");
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn edit_corrects_own_live_notes_and_points_elsewhere_otherwise() {
    let dir = scratch("edit");
    let results = session(
        &dir,
        &[
            ("relatum_init", "{}"),
            ("relatum_new", r#"{"id":"plan/split","kind":"decision","summary":"Split work","body":"a does x","author":"parent","tags":["plan"]}"#),
            ("relatum_edit", r#"{"id":"plan/split","author":"parent","summary":"Split work two ways","body":"a does x, b does y","tags":["plan","v2"]}"#),
            ("relatum_get", r#"{"ids":["plan/split"]}"#),
            ("relatum_edit", r#"{"id":"plan/split","author":"worker-a","body":"I would rather do y"}"#),
            ("relatum_edit", r#"{"id":"plan/split","author":"parent"}"#),
            ("relatum_edit", r#"{"id":"plan/split","author":"parent","tags":[]}"#),
            ("relatum_new", r#"{"id":"plan/split","kind":"decision","summary":"again","body":"b"}"#),
            ("relatum_new", r#"{"id":"plan/v3","kind":"decision","summary":"Three ways","body":"c too"}"#),
            ("relatum_supersede", r#"{"old":"plan/split","new":"plan/v3"}"#),
            ("relatum_edit", r#"{"id":"plan/split","author":"parent","body":"late fix"}"#),
            ("relatum_edit", r#"{"id":"plan/v3","author":"worker-b","summary":"Three ways, b owns c"}"#),
            ("relatum_edit", r#"{"id":"plan/v3"}"#),
            ("relatum_edit", r#"{"id":"plan/v3","author":"worker-b","summary":"  "}"#),
            ("relatum_check", "{}"),
        ],
    );
    let structured = |i: usize| results[i].get("structuredContent").and_then(|s| s.get("result")).cloned().unwrap();
    let ok = |i: usize| assert!(!is_error(&results[i]), "call {i}: {}", text(&results[i]));
    let err = |i: usize, needle: &str| {
        assert!(is_error(&results[i]), "call {i}: {}", text(&results[i]));
        assert!(text(&results[i]).contains(needle), "call {i}: {}", text(&results[i]));
    };
    ok(2);
    let today = relatum::clock::today();
    assert_eq!(structured(2).get("updated").and_then(Value::as_str), Some(today.as_str()));
    let note = structured(3).get("notes").and_then(Value::as_array).unwrap()[0].clone();
    assert_eq!(note.get("summary").and_then(Value::as_str), Some("Split work two ways"));
    assert_eq!(note.get("body").and_then(Value::as_str), Some("a does x, b does y"));
    assert_eq!(note.get("kind").and_then(Value::as_str), Some("decision"));
    assert_eq!(note.get("author").and_then(Value::as_str), Some("parent"));
    assert_eq!(json::write_compact(note.get("tags").unwrap()), r#"["plan","v2"]"#);
    err(4, "plan/split was written by parent, not worker-a; only its author edits it in place. Write a finding with your view, or write a replacement under a new id and call relatum_supersede with old set to this note.");
    err(5, "nothing to change: pass summary, body, or tags");
    ok(6);
    let file = std::fs::read_to_string(dir.join(".swarmnotes/plan/split.md")).unwrap();
    assert!(!file.contains("tags:"), "{file}");
    err(7, "note already exists: plan/split. Pick another id. To change plan/split: relatum_edit for a small fix by its author, or write the replacement under a new id and call relatum_supersede with old=plan/split.");
    ok(8);
    ok(9);
    err(10, "plan/split is superseded by plan/v3; edit the live note plan/v3 instead");
    ok(11); // plan/v3 had no author; the editor becomes its author
    let file = std::fs::read_to_string(dir.join(".swarmnotes/plan/v3.md")).unwrap();
    assert!(file.contains("author: worker-b") && file.contains("summary: Three ways, b owns c"), "{file}");
    err(12, "invalid arguments: missing required argument author. relatum_edit needs id and author.");
    err(13, "missing summary");
    ok(14);
    assert_eq!(structured(14).get("ok"), Some(&Value::Bool(true)));
    let _ = std::fs::remove_dir_all(dir);
}

/// Specula M5: summaries over 100 characters cost about six lanes a retry,
/// and the supersede that followed a failed new failed too. The limit is
/// 200 for new, edit, and check alike, it is in the schema, and the error
/// gives the length and the limit.
#[test]
fn summary_limit_is_200_in_schema_and_errors() {
    let tools = relatum::mcp_schema::tools();
    for name in ["relatum_new", "relatum_edit"] {
        let tool = tools.as_array().unwrap().iter()
            .find(|tool| tool.get("name").and_then(Value::as_str) == Some(name)).unwrap();
        let summary = tool.get("inputSchema").and_then(|s| s.get("properties")).and_then(|p| p.get("summary")).unwrap();
        assert_eq!(summary.get("maxLength"), Some(&Value::Number("200".into())), "{name}");
        assert!(summary.get("description").and_then(Value::as_str).unwrap().contains("at most 200 characters"), "{name}");
    }
    let dir = scratch("summary");
    session(&dir, &[("relatum_init", "{}")]);
    let sum = |n: usize| "s".repeat(n);
    let calls: Vec<(&str, String)> = vec![
        ("relatum_new", r#"{"id":"m5/a/old","kind":"context","summary":"old","body":"b","author":"a"}"#.into()),
        ("relatum_new", format!(r#"{{"id":"m5/a/handoff","kind":"context","summary":"{}","body":"b","author":"a"}}"#, sum(201))),
        ("relatum_supersede", r#"{"old":"m5/a/old","new":"m5/a/handoff"}"#.into()),
        ("relatum_new", format!(r#"{{"id":"m5/a/handoff","kind":"context","summary":"{}","body":"b","author":"a"}}"#, sum(200))),
        ("relatum_edit", format!(r#"{{"id":"m5/a/handoff","author":"a","summary":"{}"}}"#, sum(250))),
        ("relatum_edit", r#"{"id":"m5/a/handoff","author":"a","summary":"two\nlines"}"#.into()),
        ("relatum_check", "{}".into()),
    ];
    let borrowed: Vec<(&str, &str)> = calls.iter().map(|(name, args)| (*name, args.as_str())).collect();
    let results = session(&dir, &borrowed);
    let limit = "summary is 201 characters; the limit is 200. Shorten it and put the detail in the body";
    assert!(is_error(&results[1]) && text(&results[1]).ends_with(limit), "{}", text(&results[1]));
    assert!(is_error(&results[2]), "{}", text(&results[2]));
    assert_eq!(
        text(&results[2]),
        "Error executing tool relatum_supersede: no such note: m5/a/handoff. Write the replacement \
         first with relatum_new (summary at most 200 characters), then call relatum_supersede again."
    );
    assert!(!is_error(&results[3]), "{}", text(&results[3]));
    assert!(text(&results[4]).contains("summary is 250 characters; the limit is 200"), "{}", text(&results[4]));
    assert!(text(&results[5]).contains("summary must be one line"), "{}", text(&results[5]));
    let check = results[6].get("structuredContent").and_then(|s| s.get("result")).unwrap();
    assert_eq!(check.get("ok"), Some(&Value::Bool(true)), "{}", text(&results[6]));
    // check flags a hand-written summary past the limit with the same words.
    std::fs::write(
        dir.join(".swarmnotes/m5/a/long.md"),
        format!("---\nkind: context\nsummary: {}\nauthor: a\nupdated: 2026-11-06\n---\n\nb\n", sum(201)),
    )
    .unwrap();
    let results = session(&dir, &[("relatum_check", "{}")]);
    assert!(text(&results[0]).contains("m5/a/long: summary is 201 characters; the limit is 200"), "{}", text(&results[0]));
    let _ = std::fs::remove_dir_all(dir);
}

/// Specula M5: delta handoffs lost the earlier content, because get follows
/// superseded_by to the live note. include_superseded returns the notes each
/// live note replaced, oldest first; the default reply is unchanged.
#[test]
fn get_include_superseded_returns_the_replaced_notes() {
    let dir = scratch("history");
    let new = |id: &str, body: &str| {
        format!(r#"{{"id":"{id}","kind":"context","summary":"{id}","body":"{body}","author":"a"}}"#)
    };
    let calls: Vec<(&str, String)> = vec![
        ("relatum_init", "{}".into()),
        ("relatum_new", new("t/handoff", "round one")),
        ("relatum_new", new("t/handoff-2", "round two delta")),
        ("relatum_new", new("t/handoff-3", "round three delta")),
        ("relatum_new", new("t/side", "side note")),
        ("relatum_new", new("t/alone", "never replaced")),
        ("relatum_supersede", r#"{"old":"t/handoff","new":"t/handoff-2"}"#.into()),
        ("relatum_supersede", r#"{"old":"t/handoff-2","new":"t/handoff-3"}"#.into()),
        ("relatum_supersede", r#"{"old":"t/side","new":"t/handoff-3"}"#.into()),
        ("relatum_get", r#"{"ids":["t/handoff"]}"#.into()),
        ("relatum_get", r#"{"ids":["t/handoff","t/handoff-3","t/alone","t/missing"],"include_superseded":true}"#.into()),
        ("relatum_get", r#"{"ids":["t/handoff"],"include_superseded":"yes"}"#.into()),
    ];
    let borrowed: Vec<(&str, &str)> = calls.iter().map(|(name, args)| (*name, args.as_str())).collect();
    let results = session(&dir, &borrowed);
    for (i, result) in results.iter().enumerate().take(9) {
        assert!(!is_error(result), "call {i}: {}", text(result));
    }
    let notes = |i: usize| -> Vec<Value> {
        assert!(!is_error(&results[i]), "call {i}: {}", text(&results[i]));
        results[i].get("structuredContent").and_then(|s| s.get("result")).and_then(|r| r.get("notes"))
            .and_then(Value::as_array).unwrap().to_vec()
    };
    let plain = &notes(9)[0];
    assert!(plain.get("superseded").is_none());
    assert_eq!(json::write_compact(plain.get("chain").unwrap()), r#"["t/handoff","t/handoff-2","t/handoff-3"]"#);
    let full = notes(10);
    assert_eq!(full.len(), 3);
    let history = json::write_compact(full[0].get("superseded").unwrap());
    assert_eq!(
        history,
        r#"[{"id":"t/handoff","superseded_by":"t/handoff-2","summary":"t/handoff","body":"round one"},{"id":"t/handoff-2","superseded_by":"t/handoff-3","summary":"t/handoff-2","body":"round two delta"},{"id":"t/side","superseded_by":"t/handoff-3","summary":"t/side","body":"side note"}]"#
    );
    // Asking for the live note by its own id gives the same history.
    assert_eq!(json::write_compact(full[1].get("superseded").unwrap()), history);
    assert_eq!(json::write_compact(full[2].get("superseded").unwrap()), "[]");
    let errors = results[10].get("structuredContent").and_then(|s| s.get("result")).and_then(|r| r.get("errors")).unwrap();
    assert!(json::write_compact(errors).contains("t/missing"));
    assert!(text(&results[11]).starts_with("Error executing tool relatum_get: invalid arguments: include_superseded must be true or false"), "{}", text(&results[11]));
    let _ = std::fs::remove_dir_all(dir);
}

/// Specula M5: agents did not know when notes beat grep. The tools that
/// matter at a context refresh say so, and so do the instructions.
#[test]
fn descriptions_say_when_to_use_notes() {
    let tools = relatum::mcp_schema::tools();
    let description = |name: &str| -> String {
        tools.as_array().unwrap().iter()
            .find(|tool| tool.get("name").and_then(Value::as_str) == Some(name))
            .and_then(|tool| tool.get("description")).and_then(Value::as_str).unwrap().to_string()
    };
    for name in ["relatum_ls", "relatum_get", "relatum_new", "relatum_supersede"] {
        let text = description(name);
        assert!(text.contains("handoff") && text.contains("api note"), "{name}: {text}");
    }
    assert!(description("relatum_ls").contains("before grep"));
    let instructions = relatum::mcp_schema::instructions();
    assert!(instructions.contains("before grep") && instructions.contains("handoff") && instructions.contains("api"), "{instructions}");
}
