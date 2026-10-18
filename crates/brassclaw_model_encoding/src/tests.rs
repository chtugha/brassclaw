use super::*;
use serde_json::json;

fn tokenizer() -> String {
    json!({"version":"1.0","truncation":{"direction":"Right","max_length":4,"strategy":"LongestFirst","stride":0},
        "padding":{"strategy":{"Fixed":8},"direction":"Right","pad_to_multiple_of":null,"pad_id":0,"pad_type_id":0,"pad_token":"[UNK]"},
        "added_tokens":[],"normalizer":null,"pre_tokenizer":{"type":"WhitespaceSplit"},"post_processor":null,"decoder":null,
        "model":{"type":"WordLevel","vocab":{"[UNK]":0,"hello":1,"world":2},"unk_token":"[UNK]"}}).to_string()
}
fn request(probes: Vec<Probe>) -> Request {
    Request {
        format: PROTOCOL.into(),
        assets: Assets {
            tokenizer_json: tokenizer(),
            tokenizer_config_json: "{}".into(),
            chat_template: "{{ messages }}".into(),
        },
        probes,
    }
}
#[test]
fn full_tokens_not_saved_truncation_or_padding() {
    let report = analyze(request(vec![
        Probe::Text {
            text: "hello ".repeat(2050),
        },
        Probe::Text {
            text: "hello world".into(),
        },
    ]))
    .unwrap();
    assert_eq!(report.probes[0].token_count, 2050);
    assert_eq!(report.probes[1].token_count, 2);
    assert_eq!(
        report.probes[0].token_ids_sha256,
        ids_checksum(&vec![1; 2050])
    );
    assert_eq!(report.common_prefix_tokens, 1);
    assert_eq!(report.common_prefix_sha256, ids_checksum(&[1]));
}
#[test]
fn python_json_preserves_order_escaping_and_scalar_spelling() {
    let renderer = render::Renderer::new(
        "{{ value|tojson }}|{{ number|string }}|{{ number }}",
        context::parse("{}").unwrap(),
    )
    .unwrap();
    let rendered=renderer.render(r#"{"value":{"z":"<>&'ü😀\\u003c","a":[true,null,1.0,0.000001,10000000000000000.0,-0.0]},"number":0.000001}"#).unwrap();
    assert_eq!(
        rendered,
        r#"{"z": "<>&'ü😀\\u003c", "a": [true, null, 1.0, 1e-06, 1e+16, -0.0]}|1e-06|1e-06"#
    );
    let renderer = render::Renderer::new(
        "{{ value|tojson(ensure_ascii=true,sort_keys=true,indent=2) }}",
        context::parse("{}").unwrap(),
    )
    .unwrap();
    assert_eq!(
        renderer
            .render(r#"{"value":{"z":"😀ü","a":[1,2]}}"#)
            .unwrap(),
        "{\n  \"a\": [\n    1,\n    2\n  ],\n  \"z\": \"\\ud83d\\ude00\\u00fc\"\n}"
    );
}
#[test]
fn rejects_duplicate_deep_reserved_and_effectful_template_inputs() {
    assert!(parse_probes("[]").is_err());
    assert!(
        parse_probes(&format!(
            "[{}]",
            vec![r#"{"kind":"text","text":"x"}"#; 65].join(",")
        ))
        .is_err()
    );
    let probes = parse_probes(r#"[{"kind":"chat","context_json":"{\"z\":1,\"a\":2}"}]"#).unwrap();
    assert!(matches!(&probes[0],Probe::Chat{context_json} if context_json==r#"{"z":1,"a":2}"#));

    let renderer = render::Renderer::new(
        "{{ value|tojson(ensure_ascii=true) }}",
        context::parse("{}").unwrap(),
    )
    .unwrap();
    assert_eq!(
        renderer.render(r#"{"value":"\u007f"}"#).unwrap(),
        r#""\u007f""#
    );
    assert!(context::parse(r#"{"key":1,"key":2}"#).is_err());
    assert!(context::parse(&format!("{}0{}", "[".repeat(66), "]".repeat(66))).is_err());
    let config = context::parse("{}").unwrap();
    assert!(
        render::Renderer::new("{{ strftime_now('%Y') }}", config.clone())
            .unwrap()
            .render("{}")
            .is_err()
    );
    assert!(
        render::Renderer::new("{% include '/etc/passwd' %}", config.clone())
            .unwrap()
            .render("{}")
            .is_err()
    );
    assert!(
        render::Renderer::new("{{ range(10) }}", config)
            .unwrap()
            .render(r#"{"range":1}"#)
            .is_err()
    );
}

#[test]
fn jinja_concatenation_remains_indexable_and_sliceable() {
    let types=render::Renderer::new("{{ none is iterable }}|{{ [] is iterable }}|{{ {} is iterable }}|{{ 'x' is iterable }}|{{ 1 is iterable }}",context::parse("{}").unwrap()).unwrap();
    assert_eq!(types.render("{}").unwrap(), "False|True|True|True|False");

    let renderer=render::Renderer::new("{%- set messages = [{'role':'system'}] + messages[1:] -%}{{ messages[0].role }}|{{ messages|length }}|{% for message in messages[::-1] %}{{ message.role }},{% endfor %}",context::parse("{}").unwrap()).unwrap();
    assert_eq!(
        renderer
            .render(r#"{"messages":[{"role":"old"},{"role":"user"}]}"#)
            .unwrap(),
        "system|2|user,system,"
    );
}
