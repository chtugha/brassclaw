use brassclaw_model_encoding::{Assets, PROTOCOL, Probe, Request, analyze};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

fn oracle_python() -> Command {
    Command::new(
        std::env::var_os("BRASSCLAW_TEST_PYTHON")
            .unwrap_or_else(|| if cfg!(windows) { "python" } else { "python3" }.into()),
    )
}

#[test]
#[ignore = "requires explicitly retained local model originals and offline Python Transformers"]
fn retained_ornith_matches_huggingface() {
    let root = PathBuf::from(
        std::env::var_os("BRASSCLAW_ENCODING_MODEL_ORIGINALS")
            .expect("explicit retained model originals required"),
    );
    let read = |name| fs::read_to_string(root.join(name)).unwrap();
    let mut request = Request {
        format: PROTOCOL.into(),
        assets: Assets {
            tokenizer_json: read("tokenizer.json"),
            tokenizer_config_json: read("tokenizer_config.json"),
            chat_template: read("chat_template.jinja"),
        },
        probes: Vec::new(),
    };
    for user in ["Inspect my firewall rules.", "Explain this YAML."] {
        request.probes.push(Probe::Chat{context_json:json!({"messages":[{"role":"system","content":"Verified stable reference.\n".repeat(1200)},{"role":"user","content":user}],"add_generation_prompt":true,"enable_thinking":false}).to_string()});
    }
    // Deliberately unsorted keys and characters escaped by HTML-oriented JSON.
    request.probes.push(Probe::Chat{context_json:r#"{"messages":[{"role":"user","content":"Check the server"},{"role":"assistant","content":"","tool_calls":[{"type":"function","function":{"name":"inspect","arguments":{"z":"<>&'ü😀\\u003c","a":[true,null,1.0],"small":0.000001,"large":10000000000000000.0}}}]}],"tools":[{"type":"function","function":{"name":"inspect","description":"<>&ü","parameters":{"type":"object","properties":{"z":{"type":"string"},"a":{"type":"array"}}}}}],"add_generation_prompt":true}"#.into()});
    request.probes.push(Probe::Chat{context_json:json!({"messages":[{"role":"user","content":[{"type":"image","image":"placeholder"},{"type":"text","text":"Describe this"}]},{"role":"assistant","content":"Answer","reasoning_content":"Inspect evidence"},{"role":"tool","content":"Observation"},{"role":"user","content":"Continue"}],"add_generation_prompt":true,"enable_thinking":true}).to_string()});
    compare(request);
    // Exercise float parsing/printing independently of the model's usual prompts.
    let mut bits = 0x58f2_914e_a67d_0b3cu64;
    let mut numbers = Vec::new();
    for _ in 0..2048 {
        bits ^= bits << 13;
        bits ^= bits >> 7;
        bits ^= bits << 17;
        let value = f64::from_bits(bits);
        if value.is_finite() {
            numbers.push(value);
        }
    }
    let context = serde_json::to_string(&json!({"numbers":numbers})).unwrap();
    let scalar_request=Request {format:PROTOCOL.into(),assets:Assets{tokenizer_json:read("tokenizer.json"),tokenizer_config_json:read("tokenizer_config.json"),chat_template:"{% for value in numbers %}{{ value|tojson }}|{{ value|string }}|{{ value }}\n{% endfor %}".into()},probes:vec![Probe::Chat{context_json:context}]};
    compare(scalar_request);
}
fn compare(request: Request) {
    let encoded = serde_json::to_vec(&request).unwrap();
    let mut child = oracle_python()
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/huggingface_oracle.py"
        ))
        .env("HF_HUB_OFFLINE", "1")
        .env("TRANSFORMERS_OFFLINE", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&encoded).unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(result.status.success());
    let expected: Value = serde_json::from_slice(&result.stdout).unwrap();
    let report = analyze(request).unwrap();
    let actual = serde_json::to_value(report).unwrap();
    assert_eq!(
        actual, expected,
        "template rendering or token IDs diverged from offline Hugging Face"
    );
}

#[test]
#[ignore = "requires explicitly retained local model originals and offline Python Transformers"]
fn retained_ornith_partition_matches_huggingface() {
    let root = PathBuf::from(
        std::env::var_os("BRASSCLAW_ENCODING_MODEL_ORIGINALS")
            .expect("explicit retained model originals required"),
    );
    let read = |name| fs::read_to_string(root.join(name)).unwrap();
    let prefix = "<|im_start|>system\nVerified ünicode 😀 reference.\n<|im_end|>";
    let text = format!("{prefix}\n<|im_start|>user\nInspect my firewall.\n<|im_end|>\n");
    let probes = [prefix.len(), 0, text.len()]
        .into_iter()
        .map(|prefix_end| Probe::PartitionedText {
            text: text.clone(),
            prefix_start: 0,
            prefix_end,
        })
        .collect::<Vec<_>>();
    compare(Request {
        format: brassclaw_model_encoding::request_protocol(&probes).into(),
        assets: Assets {
            tokenizer_json: read("tokenizer.json"),
            tokenizer_config_json: read("tokenizer_config.json"),
            chat_template: read("chat_template.jinja"),
        },
        probes,
    });
}

#[test]
#[ignore = "requires explicitly retained local model originals and offline Python Transformers"]
fn retained_ornith_injected_leading_chat_matches_huggingface() {
    let root = PathBuf::from(
        std::env::var_os("BRASSCLAW_ENCODING_MODEL_ORIGINALS")
            .expect("explicit retained model originals required"),
    );
    let read = |name| fs::read_to_string(root.join(name)).unwrap();
    let reference =
        "Verified ünicode 😀 reference.\nPreserve typed inputs and original evidence.\n"
            .repeat(128);
    let mut child=oracle_python().arg("-c")
        .arg("import json,runpy,sys; data=json.load(sys.stdin); adapter=runpy.run_path(sys.argv[1]); sys.stdout.write(adapter['inject_reference'](data['reference'],data['original_template']))")
        .arg(concat!(env!("CARGO_MANIFEST_DIR"),"/../../scripts/prefix/prefix_template.py"))
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit()).spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(
            &serde_json::to_vec(&json!({
        "reference":reference,"original_template":read("chat_template.jinja")}))
            .unwrap(),
        )
        .unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(result.status.success());
    let effective_template = String::from_utf8(result.stdout).unwrap();
    // This boundary includes every reference byte and its fixed separator.
    // Framing after it remains measured in the prompt; no token is omitted.
    let leading = format!("<|im_start|>system\n{reference}\n\n");
    let tool = json!({"type":"function","function":{"name":"inspect","description":"Inspect evidence ü",
        "parameters":{"type":"object","properties":{"target":{"type":"string"}}}}});
    let contexts = vec![
        json!({"messages":[{"role":"user","content":"Inspect the firewall"}],"add_generation_prompt":true,"enable_thinking":false}),
        json!({"messages":[{"role":"system","content":"Current task rules"},{"role":"user","content":"Inspect MQTT"}],"tools":null,"add_generation_prompt":true,"enable_thinking":true}),
        json!({"messages":[{"role":"system","content":null},{"role":"user","content":"Inspect this YAML"}],"tools":[],"add_generation_prompt":true,"enable_thinking":false}),
        json!({"messages":[{"role":"system","content":[{"type":"text","text":"Task rule"}]},{"role":"user","content":"Inspect Modbus"}],"add_generation_prompt":true}),
        json!({"tools":[tool],"messages":[{"role":"system","content":"Tools are dynamic"},{"role":"user","content":"Inspect a host"}],"add_generation_prompt":true,"enable_thinking":false}),
        json!({"messages":[{"role":"user","content":format!("Repeated reference in dynamic history: {reference}")}],"add_generation_prompt":true,"enable_thinking":true}),
    ];
    let probes = contexts
        .into_iter()
        .map(|context| Probe::LeadingPrefixChat {
            context_json: context.to_string(),
            leading_prefix: leading.clone(),
        })
        .collect::<Vec<_>>();
    let request = Request {
        format: brassclaw_model_encoding::request_protocol(&probes).into(),
        assets: Assets {
            tokenizer_json: read("tokenizer.json"),
            tokenizer_config_json: read("tokenizer_config.json"),
            chat_template: effective_template,
        },
        probes,
    };
    compare(request);
}

#[test]
#[ignore = "requires explicitly retained local model originals and offline Python Transformers"]
fn retained_ornith_default_wire_matches_huggingface() {
    let root = PathBuf::from(
        std::env::var_os("BRASSCLAW_ENCODING_MODEL_ORIGINALS")
            .expect("explicit retained model originals required"),
    );
    let read = |name| fs::read_to_string(root.join(name)).unwrap();
    let model = "retained-ornith-oracle";
    let reference = "Verified ünicode 😀 reference. Preserve original evidence.";
    // Derive fixed bytes independently through the original HF template. No
    // injected/custom template or provider configuration change is involved.
    // This original template REQUIRES a user query. Use a legal whole chat and
    // verify its actual model-specific user frame; never render system-only or
    // include a guessed/user-content common prefix in the fixed bytes.
    let mut child = oracle_python()
        .arg("-c")
        .arg(include_str!("ornith_fixed_prefix_probe.py"))
        .arg(&root)
        .env("HF_HUB_OFFLINE", "1")
        .env("TRANSFORMERS_OFFLINE", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(&json!({"reference":reference})).unwrap())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let prefix = String::from_utf8(output.stdout).unwrap();
    assert!(!prefix.is_empty());
    let mapping=json!({"format":brassclaw_model_encoding::wire::WIRE_MAPPING,
        "render_defaults_json":r#"{"add_generation_prompt":true,"continue_final_message":false,"enable_thinking":false}"#,
        "missing_tools":"null","null_assistant_content":"empty_string","tool_arguments":"decode_json","text_parts":{"mode":"join","separator":"\n"},
        "forward_tool_choice":true,"forward_response_format":true,"allowed_tool_choices":["auto","none","required"],"allowed_chat_kwargs":["enable_thinking"],
        "allowed_message_fields":["role","content","tool_calls","tool_call_id","name"]}).to_string();
    let mut probes = Vec::new();
    for content in ["Inspect the firewall.", "Parse this YAML and check MQTT."] {
        probes.push(brassclaw_model_encoding::Probe::WireChat {request_json:json!({"model":model,"messages":[{"role":"system","content":[{"type":"text","text":reference}]},{"role":"user","content":[{"type":"text","text":content}]}],"max_completion_tokens":37}).to_string(),mapping_json:mapping.clone(),expected_model:model.into(),leading_prefix:prefix.clone()});
    }
    // Dynamic tools may precede system text in a default model template. This
    // probe intentionally attributes ALL input to prompt; it proves counting,
    // not that the no-tools fixed prefix can be served in that mode.
    probes.push(brassclaw_model_encoding::Probe::WireChat {request_json:json!({"model":model,"messages":[{"role":"system","content":reference},{"role":"user","content":"Inspect"},{"role":"assistant","content":null,"tool_calls":[{"id":"one","type":"function","function":{"name":"inspect","arguments":r#"{"z":"ü😀<>&","a":1}"#}}]},{"role":"tool","content":"Observed evidence","tool_call_id":"one"}],"tools":[{"type":"function","function":{"name":"inspect","description":"Inspect original ü evidence","parameters":{"type":"object","properties":{"z":{"type":"string"},"a":{"type":"integer"}}}}}],"max_tokens":41}).to_string(),mapping_json:mapping,expected_model:model.into(),leading_prefix:String::new()});
    compare(Request {
        format: brassclaw_model_encoding::request_protocol(&probes).into(),
        assets: Assets {
            tokenizer_json: read("tokenizer.json"),
            tokenizer_config_json: read("tokenizer_config.json"),
            chat_template: read("chat_template.jinja"),
        },
        probes,
    });
}
