//! Host-selected admission entry. Parsing this envelope grants no authority;
//! each task's ports still verify its selected Recipe and durable completion.
use serde::Serialize;
use serde_json::Value;

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TaskEntry {
    Chat,
    InternalRecipe { name: String },
}
impl TaskEntry {
    pub(crate) fn valid(&self) -> bool {
        match self {
            Self::Chat => true,
            Self::InternalRecipe { name } => valid_name(name),
        }
    }
}
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}
pub(crate) fn valid_value(entry: &Value) -> bool {
    let Some(entry) = entry.as_object() else {
        return false;
    };
    match entry.get("kind").and_then(Value::as_str) {
        Some("chat") => entry.len() == 1,
        Some("internal_recipe") => {
            entry.len() == 2
                && entry
                    .get("name")
                    .and_then(Value::as_str)
                    .is_some_and(valid_name)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn task_entry_accepts_only_exact_host_envelopes() {
        for entry in [
            TaskEntry::Chat,
            TaskEntry::InternalRecipe {
                name: "validator-class-21".into(),
            },
        ] {
            assert!(entry.valid());
            assert!(valid_value(&serde_json::to_value(entry).unwrap()));
        }
        for invalid in [
            json!(null),
            json!("chat"),
            json!({"kind":"unknown"}),
            json!({"kind":"chat","name":"private"}),
            json!({"kind":"internal_recipe"}),
            json!({"kind":"internal_recipe","name":null}),
            json!({"kind":"internal_recipe","name":""}),
            json!({"kind":"internal_recipe","name":"../validator"}),
            json!({"kind":"internal_recipe","name":"validator; host.call()"}),
            json!({"kind":"internal_recipe","name":"validator","approval":true}),
        ] {
            assert!(!valid_value(&invalid), "{invalid}");
        }
        assert!(
            !TaskEntry::InternalRecipe {
                name: "INVALID".into()
            }
            .valid()
        );
    }
}
