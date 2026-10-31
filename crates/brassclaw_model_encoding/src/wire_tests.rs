use super::*;
use serde_json::{Value, json};

fn mapping() -> Value {
    json!({"format":wire::WIRE_MAPPING,
        "render_defaults_json":r#"{"add_generation_prompt":true,"continue_final_message":false,"enable_thinking":false}"#,
        "missing_tools":"null","null_assistant_content":"empty_string","tool_arguments":"decode_json","text_parts":{"mode":"join","separator":"\n"},
        "forward_tool_choice":true,"forward_response_format":true,
        "allowed_tool_choices":["auto","none","required"],"allowed_chat_kwargs":["enable_thinking"],
        "allowed_message_fields":["role","content","tool_calls","tool_call_id","name"]})
}
fn body() -> Value {
    json!({"model":"retained-model","messages":[{"role":"system","content":"hello"},{"role":"user","content":"world"}],"max_tokens":17})
}
fn encode(
    body: &str,
    mapping: &str,
    prefix: &str,
    template: &str,
) -> Result<EncodingReport, EncodingError> {
    let probes = vec![Probe::WireChat {
        request_json: body.into(),
        mapping_json: mapping.into(),
        expected_model: "retained-model".into(),
        leading_prefix: prefix.into(),
    }];
    analyze(Request {format:request_protocol(&probes).into(),assets:Assets {
        tokenizer_json:json!({"version":"1.0","truncation":{"direction":"Right","max_length":1,"strategy":"LongestFirst","stride":0},
          "padding":{"strategy":{"Fixed":8},"direction":"Right","pad_to_multiple_of":null,"pad_id":0,"pad_type_id":0,"pad_token":"[UNK]"},
          "added_tokens":[],"normalizer":null,"pre_tokenizer":{"type":"WhitespaceSplit"},"post_processor":null,"decoder":null,
          "model":{"type":"WordLevel","vocab":{"[UNK]":0,"hello":1,"world":2},"unk_token":"[UNK]"}}).to_string(),
        tokenizer_config_json:"{}".into(),chat_template:template.into()},probes})
}
#[test]
fn wire_encoding_counts_one_complete_render_and_binds_original_bytes() {
    let mapping = mapping().to_string();
    let body = body().to_string();
    let report = encode(
        &body,
        &mapping,
        "hello",
        "{{ messages[0].content }} {{ messages[1].content }}",
    )
    .unwrap();
    assert_eq!(report.format, "brassclaw-model-encoding/3");
    let p = &report.probes[0];
    let split = p.partition.as_ref().unwrap();
    let wire = p.wire_input.as_ref().unwrap();
    assert_eq!(p.token_count, 2);
    assert_eq!(p.token_ids_sha256, ids_checksum(&[1, 2]));
    assert_eq!(split.prefix_tokens, 1);
    assert_eq!(split.prompt_tokens, 1);
    assert_eq!(split.prefix_token_ids_sha256, ids_checksum(&[1]));
    assert_eq!(split.prompt_token_ids_sha256, ids_checksum(&[2]));
    assert_eq!(wire.request_sha256, checksum(body.as_bytes()));
    assert_eq!(wire.mapping_sha256, checksum(mapping.as_bytes()));
    assert_eq!(wire.reserved_output_tokens, 17);
    assert_eq!(
        split.render_context_sha256.as_ref(),
        Some(&wire.render_context_sha256)
    );
    let spaced = format!(" {body}\n");
    let other = encode(
        &spaced,
        &mapping,
        "hello",
        "{{ messages[0].content }} {{ messages[1].content }}",
    )
    .unwrap();
    assert_eq!(other.probes[0].token_ids_sha256, p.token_ids_sha256);
    assert_ne!(
        other.probes[0].wire_input.as_ref().unwrap().request_sha256,
        wire.request_sha256
    );
    assert!(matches!(
        encode(
            &body,
            &mapping,
            "hell",
            "{{ messages[0].content }} {{ messages[1].content }}"
        ),
        Err(EncodingError::PartitionAlignment)
    ));
    assert!(matches!(
        encode(
            &body,
            &mapping,
            "world",
            "{{ messages[0].content }} {{ messages[1].content }}"
        ),
        Err(EncodingError::LeadingPrefixMismatch)
    ));
}
#[test]
fn wire_history_and_tool_schema_keep_order_and_explicit_normalization() {
    let raw = r#"{"model":"retained-model","messages":[{"role":"assistant","content":null,"tool_calls":[{"id":"call","type":"function","function":{"name":"inspect","arguments":"{\"z\":\"ü😀<>&\",\"a\":1}"}}]},{"role":"tool","content":"result","tool_call_id":"call"}],"tools":[{"type":"function","function":{"name":"inspect","parameters":{"type":"object","properties":{"z":{"type":"string"},"a":{"type":"integer"}}}}}],"max_completion_tokens":31,"tool_choice":"required","chat_template_kwargs":{"enable_thinking":true}}"#;
    let (context, observed) = wire::project(raw, &mapping().to_string(), "retained-model").unwrap();
    assert!(context.contains(r#""arguments":{"z":"ü😀<>&","a":1}"#));
    assert!(context.contains(r#""properties":{"z":{"type":"string"},"a":{"type":"integer"}}"#));
    let parsed: Value = serde_json::from_str(&context).unwrap();
    assert_eq!(parsed["messages"][0]["content"], "");
    assert_eq!(parsed["messages"][1]["tool_call_id"], "call");
    assert_eq!(parsed["tool_choice"], "required");
    assert_eq!(parsed["enable_thinking"], true);
    assert_eq!(observed.reserved_output_tokens, 31);
    let mut preserve = mapping();
    preserve["tool_arguments"] = json!("preserve");
    preserve["null_assistant_content"] = json!("preserve");
    let (context, _) = wire::project(raw, &preserve.to_string(), "retained-model").unwrap();
    let parsed: Value = serde_json::from_str(&context).unwrap();
    assert!(parsed["messages"][0]["content"].is_null());
    assert!(parsed["messages"][0]["tool_calls"][0]["function"]["arguments"].is_string());
}
#[test]
fn wire_rejects_unknown_interpretation_defaults_and_output_ambiguity() {
    for (key, value) in [
        ("model", json!("other")),
        ("max_tokens", json!(0)),
        ("max_tokens", json!(1.5)),
        ("max_tokens", json!("17")),
        ("max_tokens", json!(true)),
        ("max_completion_tokens", json!(18)),
        ("stream", json!(true)),
        ("n", json!(2)),
        ("reasoning_effort", json!("high")),
        ("parallel_tool_calls", json!(true)),
        ("unqualified_extension", json!("data")),
        ("chat_template_kwargs", json!({"messages":[]})),
        (
            "chat_template_kwargs",
            json!({"enable_thinking":true,"add_generation_prompt":false}),
        ),
        (
            "messages",
            json!([{"role":"user","content":[{"type":"image_url","image_url":{"url":"private"}}]}]),
        ),
    ] {
        let mut b = body();
        b[key] = value;
        assert!(
            wire::project(&b.to_string(), &mapping().to_string(), "retained-model").is_err(),
            "accepted unsupported {key}"
        );
    }
    let mut missing = body();
    missing.as_object_mut().unwrap().remove("max_tokens");
    assert!(
        wire::project(
            &missing.to_string(),
            &mapping().to_string(),
            "retained-model"
        )
        .is_err()
    );
    for defaults in [
        "{}",
        r#"{"add_generation_prompt":true,"continue_final_message":true}"#,
        r#"{"add_generation_prompt":true,"continue_final_message":false,"bos_token":"injected"}"#,
    ] {
        let mut m = mapping();
        m["render_defaults_json"] = json!(defaults);
        assert!(wire::project(&body().to_string(), &m.to_string(), "retained-model").is_err());
    }
    assert!(
        wire::project(
            r#"{"model":"retained-model","model":"retained-model"}"#,
            &mapping().to_string(),
            "retained-model"
        )
        .is_err()
    );
    let mut no_tools = body();
    no_tools["tool_choice"] = json!("required");
    assert!(
        wire::project(
            &no_tools.to_string(),
            &mapping().to_string(),
            "retained-model"
        )
        .is_err()
    );
    no_tools["tool_choice"] = json!("none");
    let (context, _) = wire::project(
        &no_tools.to_string(),
        &mapping().to_string(),
        "retained-model",
    )
    .unwrap();
    assert!(serde_json::from_str::<Value>(&context).unwrap()["tool_choice"].is_null());
}

#[test]
fn sdk_text_part_arrays_require_an_explicit_lossless_projection() {
    let raw=json!({"model":"retained-model","messages":[{"role":"system","content":[{"type":"text","text":"hello"}]},{"role":"user","content":[{"type":"text","text":"world"},{"type":"text","text":"hello"}]}],"max_tokens":7}).to_string();
    let report = encode(
        &raw,
        &mapping().to_string(),
        "hello",
        "{{ messages[0].content }} {{ messages[1].content }}",
    )
    .unwrap();
    assert_eq!(report.probes[0].token_ids_sha256, ids_checksum(&[1, 2, 1]));
    let split = report.probes[0].partition.as_ref().unwrap();
    assert_eq!(split.prefix_tokens, 1);
    assert_eq!(split.prompt_tokens, 2);
    let mut mode = mapping();
    mode["text_parts"] = json!({"mode":"reject"});
    assert!(wire::project(&raw, &mode.to_string(), "retained-model").is_err());
    mode["text_parts"] = json!({"mode":"preserve"});
    let (context, _) = wire::project(&raw, &mode.to_string(), "retained-model").unwrap();
    assert!(serde_json::from_str::<Value>(&context).unwrap()["messages"][0]["content"].is_array());
    mode["text_parts"] = json!({"mode":"join"});
    assert!(
        wire::validate_mapping(&mode.to_string()).is_err(),
        "no guessed separator"
    );
    let mut raw: Value = serde_json::from_str(&raw).unwrap();
    raw["messages"][1]["content"][0]["unqualified"] = json!(true);
    assert!(wire::project(&raw.to_string(), &mapping().to_string(), "retained-model").is_err());
}
