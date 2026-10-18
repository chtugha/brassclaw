use brassclaw_model_encoding::{Assets, PROTOCOL, Probe, Request, analyze};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

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
    let mut child = Command::new("python3")
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
