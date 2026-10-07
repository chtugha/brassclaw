//! Actual worker fixtures with separately reusable lifecycle helpers.
use serde_json::{Value, json};
mod runtime;
pub use runtime::{boot, limits, worker};

pub const SOURCE: &str =
    include_str!("../../../../crates/brassclaw_engine/orchestrator/global_mode.py");
pub fn task() -> Value {
    json!({"conversation_id": "reborn-conv-opaque",
        "message_id": "message-a", "run_id": "run-a", "turn_id": "turn-a",
        "user_input": "'quoted' Ü {{vars.query}}", "history": []})
}
