//! Installation seed sources. Execution reads the exact retained component rows.
pub(crate) const POST_TURN_INSTRUCTIONS: &str = include_str!("post_turn_instructions.txt");
pub(crate) const POST_TURN_REQUEST: &str = include_str!("post_turn_request.py");
pub(crate) fn post_turn_request_source() -> String {
    POST_TURN_REQUEST.replace(
        "\"__POST_TURN_INSTRUCTIONS__\"",
        &serde_json::json!(POST_TURN_INSTRUCTIONS.trim()).to_string(),
    )
}
