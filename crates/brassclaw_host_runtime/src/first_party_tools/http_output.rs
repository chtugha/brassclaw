use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use brassclaw_host_api::RuntimeHttpEgressResponse;
use serde_json::{Map, Value, json};

pub(super) struct HttpDispatchOutput {
    pub output: Value,
    pub network_egress_bytes: u64,
}

/// Format a complete, already sanitized strict-port result. The network owner
/// enforces the accepted response bound; formatting is not a second budget.
pub(super) fn shape_response(response: RuntimeHttpEgressResponse) -> HttpDispatchOutput {
    let mut output = Map::new();
    output.insert("status".into(), json!(response.status));
    output.insert(
        "headers".into(),
        Value::Array(
            response
                .headers
                .into_iter()
                .map(|(name, value)| json!({"name":name,"value":value}))
                .collect(),
        ),
    );
    if let Some(saved) = response.saved_body {
        output.insert(
            "saved_body".into(),
            json!({"path":saved.path.as_str(),"bytes_written":saved.bytes_written}),
        );
    } else {
        match String::from_utf8(response.body) {
            Ok(body) => {
                output.insert("body_text".into(), Value::String(body));
            }
            Err(error) => {
                output.insert(
                    "body_base64".into(),
                    Value::String(BASE64_STANDARD.encode(error.into_bytes())),
                );
            }
        }
    }
    output.insert("request_bytes".into(), json!(response.request_bytes));
    output.insert("response_bytes".into(), json!(response.response_bytes));
    output.insert(
        "redaction_applied".into(),
        json!(response.redaction_applied),
    );
    HttpDispatchOutput {
        output: Value::Object(output),
        network_egress_bytes: response.request_bytes,
    }
}
