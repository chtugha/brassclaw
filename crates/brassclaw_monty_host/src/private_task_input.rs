//! Message-free VM envelope. Fields are correlation data, never claims or grants.
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Serialize)]
pub struct PrivateTaskInput {
    format: &'static str,
    entry: &'static str,
    work_id: String,
    run_id: String,
    workflow_checksum: String,
    input_checksum: String,
}
impl PrivateTaskInput {
    pub fn new(
        work_id: Uuid,
        run_id: Uuid,
        workflow_checksum: [u8; 32],
        input_checksum: [u8; 32],
    ) -> Self {
        Self {
            format: "monty-private-task/1",
            entry: "private",
            work_id: work_id.to_string(),
            run_id: run_id.to_string(),
            workflow_checksum: hex_checksum(workflow_checksum),
            input_checksum: hex_checksum(input_checksum),
        }
    }
    pub fn run_id(&self) -> &str {
        &self.run_id
    }
}
fn hex_checksum(bytes: [u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(64);
    for byte in bytes {
        result.push(HEX[usize::from(byte >> 4)] as char);
        result.push(HEX[usize::from(byte & 15)] as char);
    }
    result
}

pub(crate) fn valid_private_value(input: &Value, with_token: bool) -> bool {
    let Some(object) = input.as_object() else {
        return false;
    };
    object.len() == if with_token { 7 } else { 6 }
        && object.get("format").and_then(Value::as_str) == Some("monty-private-task/1")
        && object.get("entry").and_then(Value::as_str) == Some("private")
        && ["work_id", "run_id"].iter().all(|name| {
            object
                .get(*name)
                .and_then(Value::as_str)
                .and_then(|id| Uuid::parse_str(id).ok())
                .is_some_and(|id| !id.is_nil())
        })
        && ["workflow_checksum", "input_checksum"].iter().all(|name| {
            object
                .get(*name)
                .and_then(Value::as_str)
                .is_some_and(|hash| {
                    hash.len() == 64
                        && hash
                            .bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                })
        })
        && (!with_token
            || object
                .get("task_token")
                .and_then(Value::as_str)
                .is_some_and(|v| !v.is_empty()))
}
