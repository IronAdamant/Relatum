//! Tool list payload. Field order matches the MCP Python SDK's JSON schema.

use crate::grammar::SUMMARY_MAX;
use crate::json::{self, Value};
use crate::prog;

pub fn instructions() -> String {
    format!(
        "\
{name_title} stores pass-by-reference notes as markdown files under .swarmnotes/.
Do not rename that directory. A note ID is its path without .md.

When you spawn subagents, write each shared fact once and send a brief:

Task: <what to do>
Read first: <note IDs>
Your scope: <directory under .swarmnotes/>
Done when: <checkable end condition>

Use these tools. They read and write the same files as the {name} CLI.
{get} follows superseded_by. {ls} hides superseded notes unless
include_superseded is true. If the working directory is not the project,
the host must set SWARMNOTES_DIR to the .swarmnotes path.
{init} creates a store only in the current working directory or at
SWARMNOTES_DIR.
{when}",
        name_title = title_name(),
        name = prog::NAME,
        get = prog::tool("get"),
        ls = prog::tool("ls"),
        init = prog::tool("init"),
        when = when_to_use(),
    )
}

/// The paragraph of the instructions that says when notes beat grep or
/// asking (1.4.0; Examen had none). Lines wrap after the tool names expand.
pub fn when_to_use() -> String {
    format!(
        "\
Reach for notes before grep or asking another agent. After a context
refresh, {ls} your scope and {get} your handoff note. Before
you stop, write one ({new}, or {supersede} the last). Put what
other agents call in an api note (e.g. <scope>/api); read theirs the same
way.
",
        ls = prog::tool("ls"),
        get = prog::tool("get"),
        new = prog::tool("new"),
        supersede = prog::tool("supersede"),
    )
}

fn title_name() -> String {
    let mut chars = prog::NAME.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

pub fn tools() -> Value {
    let supersede = format!(
        "\
Mark old as replaced by new, e.g. a fresh handoff or api note replacing the\n\
last one. Write new with {new} first; it must exist and resolve to a live note.\n\
\n\
Later {get} calls on old follow the chain (include_superseded also returns\n\
old's body). {ls} hides old unless include_superseded is true.\n",
        new = prog::tool("new"),
        get = prog::tool("get"),
        ls = prog::tool("ls"),
    );
    let new = format!(
        "\
Create a note. Write one for any fact more than one agent needs, so briefs\n\
and reports cite its id instead of pasting it: a handoff before your context\n\
runs out or your work ends (state, open items, gotchas), an api note when\n\
other agents will call your code, a finding or attempt from your work.\n\
\n\
Fails if the id already exists; never overwrites. To replace a note, write\n\
the new one, then call {supersede}. kind is decision, constraint,\n\
convention, context, format, finding, or attempt. summary is one line, at\n\
most {max} characters. body is the fact (keep it under ~300 words). tags is\n\
a list of slugs.\n",
        supersede = prog::tool("supersede"),
        max = SUMMARY_MAX,
    );
    json::arr(vec![
        tool("init", INIT, init_schema(), false),
        tool("ls", LS, ls_schema(), false),
        tool("get", GET, get_schema(), true),
        tool("new", &new, new_schema(), true),
        tool("edit", &edit_description(), edit_schema(), true),
        tool("supersede", &supersede, supersede_schema(), true),
        tool("check", CHECK, check_schema(), false),
    ])
}

fn tool(suffix: &str, description: &str, input: Value, _required: bool) -> Value {
    let name = prog::tool(suffix);
    json::obj(vec![
        ("description", json::s(description)),
        ("inputSchema", input),
        ("name", json::s(&name)),
        ("outputSchema", output_schema(&name)),
    ])
}

fn output_schema(name: &str) -> Value {
    json::obj(vec![
        (
            "properties",
            json::obj(vec![(
                "result",
                json::obj(vec![
                    ("additionalProperties", json::bool_val(true)),
                    ("title", json::s("Result")),
                    ("type", json::s("object")),
                ]),
            )]),
        ),
        ("required", json::arr(vec![json::s("result")])),
        ("type", json::s("object")),
        ("title", json::s(&format!("{name}Output"))),
    ])
}

fn schema(name: &str, properties: Value, required: &[&str]) -> Value {
    let mut pairs = vec![("properties", properties)];
    if !required.is_empty() {
        pairs.push((
            "required",
            json::arr(required.iter().map(|item| json::s(item)).collect()),
        ));
    }
    pairs.push(("type", json::s("object")));
    pairs.push(("title", json::s(&format!("{name}Arguments"))));
    json::obj(pairs)
}

fn str_prop(title: &str, default: Option<&str>) -> Value {
    let mut pairs = Vec::new();
    if let Some(default) = default {
        pairs.push(("default", json::s(default)));
    }
    pairs.push(("title", json::s(title)));
    pairs.push(("type", json::s("string")));
    json::obj(pairs)
}

fn optional_str(title: &str) -> Value {
    json::obj(vec![
        (
            "anyOf",
            json::arr(vec![
                json::obj(vec![("type", json::s("string"))]),
                json::obj(vec![("type", json::s("null"))]),
            ]),
        ),
        ("default", Value::Null),
        ("title", json::s(title)),
    ])
}

fn bool_prop(title: &str) -> Value {
    json::obj(vec![
        ("default", json::bool_val(false)),
        ("title", json::s(title)),
        ("type", json::s("boolean")),
    ])
}

fn init_schema() -> Value {
    schema(
        &prog::tool("init"),
        json::obj(vec![("path", str_prop("Path", Some(".")))]),
        &[],
    )
}

fn ls_schema() -> Value {
    schema(
        &prog::tool("ls"),
        json::obj(vec![
            ("scope", str_prop("Scope", Some(""))),
            ("kind", optional_str("Kind")),
            ("include_superseded", bool_prop("Include Superseded")),
            ("tag", optional_str("Tag")),
            (
                "limit",
                described(
                    optional_typed("Limit", "integer"),
                    "Return at most this many notes. When notes are left out, the result has more (how many).",
                ),
            ),
            (
                "since",
                described(
                    optional_str("Since"),
                    "Only notes written or changed at or after this time: a date such as 2026-11-05 \
                     (from local midnight, the calendar updated uses) or a time such as \
                     2026-11-05T14:30:00Z or 2026-11-05T14:30:00+11:00 (no zone means local). \
                     Pass a modified value from an earlier details listing to see what is new.",
                ),
            ),
            (
                "details",
                described(
                    bool_prop("Details"),
                    "Also return author, updated, and modified (file time, UTC) per note.",
                ),
            ),
        ]),
        &[],
    )
}

/// A property schema with a description, for arguments an LLM fills.
fn described(property: Value, description: &str) -> Value {
    let Value::Object(mut pairs) = property else {
        return property;
    };
    pairs.insert(0, ("description".to_string(), json::s(description)));
    Value::Object(pairs)
}

fn optional_typed(title: &str, type_name: &str) -> Value {
    json::obj(vec![
        (
            "anyOf",
            json::arr(vec![
                json::obj(vec![("type", json::s(type_name))]),
                json::obj(vec![("type", json::s("null"))]),
            ]),
        ),
        ("default", Value::Null),
        ("title", json::s(title)),
    ])
}

fn get_schema() -> Value {
    schema(
        &prog::tool("get"),
        json::obj(vec![
            (
                "ids",
                json::obj(vec![
                    (
                        "items",
                        json::obj(vec![("type", json::s("string"))]),
                    ),
                    ("title", json::s("Ids")),
                    ("type", json::s("array")),
                ]),
            ),
            (
                "include_superseded",
                described(
                    bool_prop("Include Superseded"),
                    "Also return superseded: the notes each live note replaced (id, superseded_by, \
                     summary, body), oldest first. For handoffs written as deltas.",
                ),
            ),
        ]),
        &["ids"],
    )
}

/// A summary argument: its limit is in the schema and the description.
fn summary_prop(property: Value, description: &str) -> Value {
    let Value::Object(mut pairs) = described(property, description) else {
        unreachable!("described keeps an object")
    };
    pairs.insert(1, ("maxLength".to_string(), Value::Number(SUMMARY_MAX.to_string())));
    Value::Object(pairs)
}

fn new_schema() -> Value {
    schema(
        &prog::tool("new"),
        json::obj(vec![
            ("id", str_prop("Id", None)),
            ("kind", str_prop("Kind", None)),
            (
                "summary",
                summary_prop(
                    str_prop("Summary", None),
                    &format!("One line, at most {SUMMARY_MAX} characters; shown in listings."),
                ),
            ),
            ("body", str_prop("Body", None)),
            ("author", optional_str("Author")),
            (
                "tags",
                json::obj(vec![
                    (
                        "anyOf",
                        json::arr(vec![
                            json::obj(vec![
                                (
                                    "items",
                                    json::obj(vec![("type", json::s("string"))]),
                                ),
                                ("type", json::s("array")),
                            ]),
                            json::obj(vec![("type", json::s("null"))]),
                        ]),
                    ),
                    ("default", Value::Null),
                    ("title", json::s("Tags")),
                ]),
            ),
        ]),
        &["id", "kind", "summary", "body"],
    )
}

fn edit_description() -> String {
    format!(
        "\
Correct a note in place: replace its summary, body, or tags; updated becomes today.\n\
\n\
For small fixes to a note you wrote (author must match the note's author).\n\
To replace a note, or to change one someone else wrote, write a new note and\n\
call {supersede} with old set to this note. Superseded notes cannot be edited.\n",
        supersede = prog::tool("supersede"),
    )
}

fn edit_schema() -> Value {
    let tags = json::obj(vec![
        (
            "anyOf",
            json::arr(vec![
                json::obj(vec![
                    ("items", json::obj(vec![("type", json::s("string"))])),
                    ("type", json::s("array")),
                ]),
                json::obj(vec![("type", json::s("null"))]),
            ]),
        ),
        ("default", Value::Null),
        ("title", json::s("Tags")),
    ]);
    schema(
        &prog::tool("edit"),
        json::obj(vec![
            ("id", described(str_prop("Id", None), "The note to correct.")),
            (
                "author",
                described(str_prop("Author", None), "Who is editing; must be the note's author."),
            ),
            (
                "summary",
                summary_prop(
                    optional_str("Summary"),
                    &format!("New one-line summary, at most {SUMMARY_MAX} characters."),
                ),
            ),
            ("body", described(optional_str("Body"), "New body; replaces the whole body.")),
            ("tags", described(tags, "New tag slugs; [] removes the tags.")),
        ]),
        &["id", "author"],
    )
}

fn supersede_schema() -> Value {
    schema(
        &prog::tool("supersede"),
        json::obj(vec![
            ("old", str_prop("Old", None)),
            ("new", str_prop("New", None)),
        ]),
        &["old", "new"],
    )
}

fn check_schema() -> Value {
    schema(&prog::tool("check"), json::obj(vec![]), &[])
}

const INIT: &str = "\
Create the default store and its _shared/report-format note. Call it when\n\
another tool reports that no store exists.\n\
\n\
Idempotent. path must be the current working directory, the directory in\n\
SWARMNOTES_DIR, or the project directory whose .swarmnotes is\n\
SWARMNOTES_DIR. Any other path is an error. Returns root (the store path)\n\
and report_format (its note id).\n";

const LS: &str = "\
List notes as id, kind, summary, and status. Use it before grep or asking:\n\
after a context refresh, list your scope to find your handoff note; before\n\
using another agent's code, list its scope for an api note.\n\
\n\
scope is a directory under the store (e.g. m5/editor); empty lists everything. kind filters\n\
to one of decision, constraint, convention, context, format, finding,\n\
attempt. Superseded notes are omitted unless include_superseded is true.\n\
tag filters to one slug. The store is .swarmnotes/ or $SWARMNOTES_DIR.\n";

const GET: &str = "\
Fetch one or more notes by ID: the notes a brief lists under Read first,\n\
your handoff after a context refresh, or another agent's api note before\n\
you call its code. Follows superseded_by, including multi-hop chains.\n\
\n\
Returns notes (the live note, with chain: the ids followed) and errors (ids\n\
that failed). Other ids are still returned when one id is missing.\n\
include_superseded adds the bodies of the notes each live note replaced.\n";

const CHECK: &str = "\
Lint every note in the store. Run it after editing note files by hand or\n\
before committing .swarmnotes/.\n\
\n\
ok is false when there are errors. warnings (long bodies, missing author\n\
or updated) do not fail the check. errors and warnings are the same\n\
strings the CLI prints.\n";
