//! Notes given as JSON: the `notes` array of `relatum_new` and the file of
//! `new --batch`. Both take the same objects.

use crate::json::Value;
use crate::mcp_support::received;
use crate::ops::{NewNote, TagsIn};

/// The fields one note object may carry.
pub const FIELDS: [&str; 7] = ["id", "kind", "summary", "body", "author", "tags", "supersedes"];
const REQUIRED: [&str; 4] = ["id", "kind", "summary", "body"];

/// A note object that is not the right shape. `field` is the argument path
/// (`notes[2].kind`); `example` is set for a value of the wrong type.
#[derive(Debug)]
pub struct ItemError {
    pub field: String,
    pub text: String,
    pub example: Option<&'static str>,
}

/// Every note in `items`, or the first item that is not a valid note object.
pub fn notes_from_json(items: &[Value]) -> Result<Vec<NewNote>, ItemError> {
    items
        .iter()
        .enumerate()
        .map(|(index, item)| note_from_json(index, item))
        .collect()
}

fn note_from_json(index: usize, item: &Value) -> Result<NewNote, ItemError> {
    let at = format!("notes[{index}]");
    let Value::Object(pairs) = item else {
        return Err(ItemError {
            text: format!("{at} must be an object with id, kind, summary, and body, got {}", received(item)),
            field: at,
            example: None,
        });
    };
    if let Some((key, _)) = pairs.iter().find(|(key, _)| !FIELDS.contains(&key.as_str())) {
        return Err(ItemError {
            text: format!("{at} has unknown field {key}; a note takes {}", FIELDS.join(", ")),
            field: format!("{at}.{key}"),
            example: None,
        });
    }
    let missing: Vec<&str> = REQUIRED
        .iter()
        .copied()
        .filter(|key| !pairs.iter().any(|(name, _)| name == key))
        .collect();
    if !missing.is_empty() {
        return Err(ItemError {
            text: format!("{at} is missing {}; each note needs id, kind, summary, and body", missing.join(", ")),
            field: at,
            example: None,
        });
    }
    let get = |key: &str| item.get(key);
    let wrong = |key: &str, want: &str, example: &'static str, value: &Value| ItemError {
        field: format!("{at}.{key}"),
        text: format!("{at}.{key} must be {want}, got {}", received(value)),
        example: Some(example),
    };
    let text = |key: &str| -> Result<String, ItemError> {
        match get(key) {
            Some(Value::String(text)) => Ok(text.clone()),
            Some(value) => Err(wrong(key, "a string", "\"text\"", value)),
            None => Ok(String::new()),
        }
    };
    let optional = |key: &str| -> Result<Option<String>, ItemError> {
        match get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(text)) => Ok(Some(text.clone())),
            Some(value) => Err(wrong(key, "a string", "\"text\"", value)),
        }
    };
    let tags = match get("tags") {
        None | Some(Value::Null) => None,
        Some(Value::Array(items)) => {
            let mut out = Vec::new();
            for tag in items {
                match tag {
                    Value::String(text) => out.push(text.clone()),
                    _ => return Err(wrong("tags", "a list of strings", "[\"net\"]", get("tags").unwrap_or(&Value::Null))),
                }
            }
            Some(TagsIn::List(out))
        }
        Some(value) => return Err(wrong("tags", "a list of strings", "[\"net\"]", value)),
    };
    Ok(NewNote {
        id: text("id")?,
        kind: text("kind")?,
        summary: text("summary")?,
        body: text("body")?,
        author: optional("author")?,
        tags,
        supersedes: optional("supersedes")?,
    })
}
