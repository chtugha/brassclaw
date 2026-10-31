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

fn partition_request(text: &str, start: usize, end: usize) -> Request {
    let mut request = request(vec![Probe::PartitionedText {
        text: text.into(),
        prefix_start: start,
        prefix_end: end,
    }]);
    request.format = request_protocol(&request.probes).into();
    request
}
#[test]
fn partitions_actual_full_text_ids_without_saved_padding_or_truncation() {
    for (start, end, prefix_ids, prompt_ids) in [
        (0, 5, vec![1], vec![2]),
        (6, 11, vec![2], vec![1]),
        (0, 11, vec![1, 2], vec![]),
        (0, 0, vec![], vec![1, 2]),
    ] {
        let report = analyze(partition_request("hello world", start, end)).unwrap();
        assert_eq!(report.format, "brassclaw-model-encoding/2");
        let probe = &report.probes[0];
        let partition = probe.partition.as_ref().unwrap();
        assert_eq!(probe.token_ids_sha256, ids_checksum(&[1, 2]));
        assert_eq!(partition.prefix_tokens, prefix_ids.len() as u64);
        assert_eq!(partition.prompt_tokens, prompt_ids.len() as u64);
        assert_eq!(
            partition.prefix_tokens + partition.prompt_tokens,
            probe.token_count
        );
        assert_eq!(partition.prefix_token_ids_sha256, ids_checksum(&prefix_ids));
        assert_eq!(partition.prompt_token_ids_sha256, ids_checksum(&prompt_ids));
        assert_eq!(
            partition.prefix_bytes_sha256,
            checksum(&b"hello world"[start..end])
        );
        assert_eq!(partition.qualification, "supplied-byte-span-only");
    }
    let report = analyze(partition_request(&"hello ".repeat(2050), 0, 5)).unwrap();
    assert_eq!(report.probes[0].token_count, 2050);
    assert_eq!(
        report.probes[0].partition.as_ref().unwrap().prompt_tokens,
        2049
    );
}
#[test]
fn physical_partition_rejects_boundary_merges_and_invalid_utf8_ranges() {
    for (text, start, end) in [("hello world", 1, 5), ("hello world", 0, 4)] {
        assert!(matches!(
            analyze(partition_request(text, start, end)),
            Err(EncodingError::PartitionAlignment)
        ));
    }
    for (text, start, end) in [
        ("ü world", 0, 1),
        ("hello", 5, 4),
        ("hello", 0, 6),
        ("hello", 2, 2),
    ] {
        assert!(matches!(
            analyze(partition_request(text, start, end)),
            Err(EncodingError::InvalidInput)
        ));
    }
    let report = analyze(partition_request("ü world", 0, 2)).unwrap();
    assert_eq!(
        report.probes[0].partition.as_ref().unwrap().prefix_tokens,
        1
    );
    assert_eq!(
        report.probes[0].partition.as_ref().unwrap().prompt_tokens,
        1
    );
}
#[test]
fn partition_requires_new_protocol_and_legacy_report_shape_stays_exact() {
    let mut new = partition_request("hello world", 0, 5);
    new.format = PROTOCOL.into();
    assert!(matches!(analyze(new), Err(EncodingError::InvalidInput)));
    let mut old = request(vec![Probe::Text {
        text: "hello world".into(),
    }]);
    old.format = PARTITION_PROTOCOL.into();
    assert!(matches!(analyze(old), Err(EncodingError::InvalidInput)));
    let legacy = serde_json::to_value(
        analyze(request(vec![Probe::Text {
            text: "hello world".into(),
        }]))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(legacy["format"], "brassclaw-model-encoding/1");
    assert_eq!(legacy["probes"][0].as_object().unwrap().len(), 4);
    assert!(legacy["probes"][0].get("partition").is_none());
    assert!(parse_probes(r#"[{"kind":"partitioned_text","text":"hello","prefix_start":0,"prefix_end":5,"claim":"semantic"}]"#).is_err());
}

#[test]
fn actual_bpe_boundary_merge_is_not_independent_substring_counting() {
    let model = tokenizers::models::bpe::BPE::builder()
        .vocab_and_merges(
            [
                ("a".to_owned(), 0),
                ("b".to_owned(), 1),
                ("ab".to_owned(), 2),
            ],
            vec![("a".to_owned(), "b".to_owned())],
        )
        .build()
        .unwrap();
    let tokenizer = Tokenizer::new(model);
    assert_eq!(tokenizer.encode("a", false).unwrap().len(), 1);
    assert_eq!(tokenizer.encode("b", false).unwrap().len(), 1);
    assert_eq!(tokenizer.encode("ab", false).unwrap().get_ids(), &[2]);
    let mut request = partition_request("ab", 0, 1);
    request.assets.tokenizer_json = tokenizer.to_string(false).unwrap();
    assert!(matches!(
        analyze(request),
        Err(EncodingError::PartitionAlignment)
    ));
}
#[test]
fn unmappable_offsets_fail_closed_but_disabled_prefix_counts_all_ids() {
    for offsets in [(0, 0), (4, 3), (0, 6)] {
        let encoding = tokenizers::Encoding::from_tokens(
            vec![tokenizers::Token {
                id: 7,
                value: "opaque".into(),
                offsets,
            }],
            0,
        );
        assert!(matches!(
            partition_tokens("hello", 0, 5, &encoding),
            Err(EncodingError::PartitionAlignment)
        ));
        let disabled = partition_tokens("hello", 0, 0, &encoding).unwrap();
        assert_eq!(disabled.prefix_tokens, 0);
        assert_eq!(disabled.prompt_tokens, 1);
        assert_eq!(disabled.prompt_token_ids_sha256, ids_checksum(&[7]));
    }
}

fn leading_request(context: &str, prefix: &str) -> Request {
    let probes = vec![Probe::LeadingPrefixChat {
        context_json: context.into(),
        leading_prefix: prefix.into(),
    }];
    Request {
        format: request_protocol(&probes).into(),
        assets: Assets {
            tokenizer_json: tokenizer(),
            tokenizer_config_json: "{}".into(),
            chat_template: "{{ messages[0].content }}".into(),
        },
        probes,
    }
}
#[test]
fn leading_chat_binds_actual_full_render_context_and_partition() {
    let context = r#"{"messages":[{"role":"system","content":"hello world"}]}"#;
    let report = analyze(leading_request(context, "hello")).unwrap();
    let probe = &report.probes[0];
    let partition = probe.partition.as_ref().unwrap();
    assert_eq!(report.format, "brassclaw-model-encoding/2");
    assert_eq!(probe.rendered_sha256, checksum(b"hello world"));
    assert_eq!(partition.prefix_start, 0);
    assert_eq!(partition.prefix_end, 5);
    assert_eq!(partition.prefix_tokens, 1);
    assert_eq!(partition.prompt_tokens, 1);
    assert_eq!(
        partition.qualification,
        "verified-leading-render-bytes-only"
    );
    assert_eq!(
        partition.render_context_sha256.as_deref(),
        Some(checksum(context.as_bytes()).as_str())
    );
    let unicode = r#"{"messages":[{"role":"system","content":"ü world"}]}"#;
    assert_eq!(
        analyze(leading_request(unicode, "ü")).unwrap().probes[0]
            .partition
            .as_ref()
            .unwrap()
            .prefix_end,
        2
    );
    let disabled = analyze(leading_request(unicode, "")).unwrap();
    assert_eq!(
        disabled.probes[0].partition.as_ref().unwrap().prefix_tokens,
        0
    );
    assert_eq!(
        disabled.probes[0].partition.as_ref().unwrap().prompt_tokens,
        2
    );
}
#[test]
fn leading_chat_never_searches_repeated_later_text_or_accepts_boundary_merges() {
    let context = r#"{"messages":[{"role":"system","content":"hello world hello"}]}"#;
    assert!(matches!(
        analyze(leading_request(context, "world")),
        Err(EncodingError::LeadingPrefixMismatch)
    ));
    assert!(matches!(
        analyze(leading_request(context, "hell")),
        Err(EncodingError::PartitionAlignment)
    ));
    let report = analyze(leading_request(context, "hello")).unwrap();
    let partition = report.probes[0].partition.as_ref().unwrap();
    assert_eq!(partition.prefix_tokens, 1);
    assert_eq!(partition.prompt_tokens, 2);
    assert_eq!(partition.prefix_token_ids_sha256, ids_checksum(&[1]));
    assert_eq!(partition.prompt_token_ids_sha256, ids_checksum(&[2, 1]));
    assert!(matches!(
        analyze(leading_request("{bad}", "hello")),
        Err(EncodingError::InvalidInput)
    ));
    let mut old = leading_request(context, "hello");
    old.format = PROTOCOL.into();
    assert!(matches!(analyze(old), Err(EncodingError::InvalidInput)));
}
