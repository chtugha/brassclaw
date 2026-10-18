use brassclaw_host_api::JsonValue;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

static_assertions::assert_impl_all!(JsonValue: Send, Sync, Clone, Eq);
static_assertions::assert_not_impl_any!(JsonValue: std::ops::DerefMut);

#[test]
fn shared_json_preserves_bytes_and_keeps_borrowed_data_immutable() {
    let original = json!({"secret":"private result", "é":[null,true,-1,u64::MAX,1.0,"ß\u{0000}\\\""],"empty":{}});
    let shared = JsonValue::from(original.clone());
    let other = shared.clone();
    assert!(std::ptr::eq(shared.as_value(), other.as_value()));
    assert_eq!(shared, original);
    assert_eq!(original, shared);
    assert_eq!(
        serde_json::to_string(&shared).unwrap(),
        serde_json::to_string(&original).unwrap()
    );
    assert_eq!(shared.to_string(), original.to_string());
    assert_eq!(format!("{shared:#}"), format!("{original:#}"));
    assert!(!format!("{shared:?}").contains("private result"));
    let decoded: JsonValue = serde_json::from_str(&shared.to_string()).unwrap();
    assert_eq!(shared, decoded);
    assert_ne!(shared, JsonValue::from(json!({"other":1})));
    assert!(serde_json::from_str::<JsonValue>(r#"{"a":true,"\u0061":false}"#).is_err());
}

#[test]
fn deep_shared_data_clones_serializes_and_frees_on_a_small_stack() {
    std::thread::Builder::new()
        .stack_size(64 * 1024)
        .spawn(|| {
            let depth = 12_000;
            let mut value = Value::Bool(true);
            for _ in 0..depth {
                value = Value::Array(vec![value]);
            }
            let shared = JsonValue::from(value);
            let clone = shared.clone();
            assert!(std::ptr::eq(shared.as_value(), clone.as_value()));
            let source = format!("{}true{}", "[".repeat(depth), "]".repeat(depth));
            assert_eq!(serde_json::to_string(&shared).unwrap(), source);
            assert_eq!(shared.to_string(), source);
            let mut decoder = serde_json::Deserializer::from_str(&source);
            // Test-only direct codec qualification, not live protocol admission.
            decoder.disable_recursion_limit();
            let decoded = JsonValue::deserialize(&mut decoder).unwrap();
            decoder.end().unwrap();
            assert_eq!(shared, decoded);
            assert!(!std::ptr::eq(shared.as_value(), decoded.as_value()));
            drop(shared);
            drop(clone);
            drop(decoded);
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn partial_syntax_and_later_envelope_failure_free_completed_deep_fields() {
    std::thread::Builder::new()
        .stack_size(64 * 1024)
        .spawn(|| {
            let depth = 12_000;
            let data = format!("{}true{}", "[".repeat(depth), "]".repeat(depth));
            #[derive(Deserialize, Serialize)]
            #[serde(deny_unknown_fields)]
            struct Envelope {
                data: JsonValue,
                stamp: u8,
            }
            for invalid in [
                format!("[{data},]"),
                format!("{{\"a\":{data},\"a\":false}}"),
            ] {
                let mut decoder = serde_json::Deserializer::from_str(&invalid);
                decoder.disable_recursion_limit();
                assert!(JsonValue::deserialize(&mut decoder).is_err());
            }
            for invalid in [
                format!("{{\"data\":{data},\"stamp\":\"invalid\"}}"),
                format!("{{\"data\":{data},\"stamp\":1,\"extra\":true}}"),
            ] {
                let mut decoder = serde_json::Deserializer::from_str(&invalid);
                decoder.disable_recursion_limit();
                assert!(Envelope::deserialize(&mut decoder).is_err());
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
