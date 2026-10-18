//! Parse answers without Serde's internally tagged Content buffer: a completed
//! data field remains guarded if a later field/tag fails. Wire shapes stay the
//! same. The enclosing process protocol still has its existing depth gate.
use std::fmt;

use brassclaw_host_api::JsonValue;
use serde::{
    Deserialize, Deserializer,
    de::{MapAccess, Visitor},
};
use serde_json::Value;

use super::PortAnswer;
use crate::{HostAnswer, MontyException};

struct WireAnswer {
    kind: String,
    value: Option<JsonValue>,
    reason: Option<String>,
}

impl<'de> Deserialize<'de> for WireAnswer {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct AnswerVisitor;
        impl<'de> Visitor<'de> for AnswerVisitor {
            type Value = WireAnswer;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a classified host answer")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let (mut kind, mut value, mut reason) = (None, None, None);
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "kind" if kind.is_none() => kind = Some(map.next_value::<String>()?),
                        "value" if value.is_none() => value = Some(map.next_value::<JsonValue>()?),
                        "reason_kind" if reason.is_none() => {
                            reason = Some(map.next_value::<String>()?)
                        }
                        _ => return Err(serde::de::Error::custom("invalid host answer fields")),
                    }
                }
                Ok(WireAnswer {
                    kind: kind
                        .ok_or_else(|| serde::de::Error::custom("missing host answer kind"))?,
                    value,
                    reason,
                })
            }
        }
        deserializer.deserialize_map(AnswerVisitor)
    }
}

impl<'de> Deserialize<'de> for PortAnswer {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let answer = WireAnswer::deserialize(deserializer)?;
        match (answer.kind.as_str(), answer.value, answer.reason) {
            ("return", Some(value), None) => Ok(Self::Return { value }),
            ("domain_error", None, Some(reason_kind)) => Ok(Self::DomainError { reason_kind }),
            ("terminal_error", None, Some(reason_kind)) => Ok(Self::TerminalError { reason_kind }),
            _ => Err(serde::de::Error::custom("invalid host answer")),
        }
    }
}

fn exception(value: &JsonValue) -> Option<MontyException> {
    // Pinned MontyException metadata has no recursive payload type. Its deepest
    // valid shape is Unicode bytes at depth five; reject deeper forged metadata
    // before the upstream enum visitor could buffer it. This is wire-schema
    // validation, independent of the task's arbitrary JSON data depth setting.
    let mut pending = vec![(value.as_value(), 0usize)];
    while let Some((value, depth)) = pending.pop() {
        if depth > 5 {
            return None;
        }
        match value {
            Value::Array(values) => pending.extend(values.iter().map(|value| (value, depth + 1))),
            Value::Object(values) => {
                pending.extend(values.values().map(|value| (value, depth + 1)))
            }
            _ => {}
        }
    }
    MontyException::deserialize(serde_stacker::Deserializer::new(value.as_value())).ok()
}

impl<'de> Deserialize<'de> for HostAnswer {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let answer = WireAnswer::deserialize(deserializer)?;
        if answer.reason.is_some() {
            return Err(serde::de::Error::custom("invalid host answer"));
        }
        match (answer.kind.as_str(), answer.value) {
            ("Return", Some(value)) => Ok(Self::Return(value)),
            ("Raise", Some(value)) => exception(&value)
                .map(Self::Raise)
                .ok_or_else(|| serde::de::Error::custom("invalid host exception")),
            ("Abort", Some(value)) => exception(&value)
                .map(Self::Abort)
                .ok_or_else(|| serde::de::Error::custom("invalid host exception")),
            _ => Err(serde::de::Error::custom("invalid host answer")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ExcType;
    use serde_json::json;

    #[test]
    fn answer_wire_shapes_and_strict_fields_remain_exact() {
        let answer = PortAnswer::Return {
            value: json!({"é":[1,null]}).into(),
        };
        let bytes = serde_json::to_string(&answer).unwrap();
        assert_eq!(bytes, r#"{"kind":"return","value":{"é":[1,null]}}"#);
        let PortAnswer::Return { value } = serde_json::from_str(&bytes).unwrap() else {
            panic!("return required")
        };
        assert_eq!(value, json!({"é":[1,null]}));
        for source in [
            r#"{"value":null,"kind":"return"}"#,
            r#"{"kind":"return","value":null}"#,
        ] {
            assert!(
                matches!(serde_json::from_str::<PortAnswer>(source).unwrap(), PortAnswer::Return { value } if value.is_null())
            );
        }
        for source in [
            r#"{"kind":"return"}"#,
            r#"{"kind":"return","value":null,"value":true}"#,
            r#"{"kind":"return","kind":"return","value":null}"#,
            r#"{"kind":"return","value":null,"reason_kind":"bad"}"#,
            r#"{"kind":"domain_error","value":null,"reason_kind":"bad"}"#,
            r#"{"kind":"terminal_error","reason_kind":null}"#,
            r#"{"kind":"return","value":{},"extra":true}"#,
        ] {
            assert!(serde_json::from_str::<PortAnswer>(source).is_err());
        }
        for answer in [
            HostAnswer::Return(json!([true]).into()),
            HostAnswer::Raise(MontyException::new(
                ExcType::ValueError,
                Some("safe message".into()),
            )),
            HostAnswer::Abort(MontyException::new(ExcType::RuntimeError, None)),
        ] {
            let expected = serde_json::to_string(&answer).unwrap();
            let decoded: HostAnswer = serde_json::from_str(&expected).unwrap();
            assert_eq!(serde_json::to_string(&decoded).unwrap(), expected);
        }
        let unicode = MontyException::new(ExcType::UnicodeDecodeError, None).with_data(
            monty_types::ExcData::Unicode(Box::new(monty_types::UnicodeErrorData {
                encoding: "utf-8".into(),
                object: monty_types::UnicodeErrorObject::Bytes(vec![255, 0, 1]),
                start: 0,
                end: 1,
                reason: "invalid byte".into(),
            })),
        );
        let source = serde_json::to_string(&HostAnswer::Raise(unicode.clone())).unwrap();
        let HostAnswer::Raise(actual) = serde_json::from_str(&source).unwrap() else {
            panic!("raise required")
        };
        assert_eq!(actual, unicode);
        let error = serde_json::from_str::<PortAnswer>(
            r#"{"kind":"domain_error","reason_kind":"failure"}"#,
        )
        .unwrap();
        assert_eq!(
            serde_json::to_string(&error).unwrap(),
            r#"{"kind":"domain_error","reason_kind":"failure"}"#
        );
    }

    #[test]
    fn deep_return_and_late_answer_errors_keep_owned_data_stack_safe() {
        std::thread::Builder::new()
            .stack_size(64 * 1024)
            .spawn(|| {
                let depth = 12_000;
                let data = format!("{}true{}", "[".repeat(depth), "]".repeat(depth));
                let source = format!("{{\"kind\":\"return\",\"value\":{data}}}");
                let mut decoder = serde_json::Deserializer::from_str(&source);
                // Test only: outer process framing/admission still enforces64.
                decoder.disable_recursion_limit();
                let answer = PortAnswer::deserialize(&mut decoder).unwrap();
                decoder.end().unwrap();
                let PortAnswer::Return { value } = answer else {
                    panic!("return required")
                };
                let retained = value.clone();
                assert!(std::ptr::eq(value.as_value(), retained.as_value()));
                drop(value);
                assert_eq!(
                    serde_json::to_string(&PortAnswer::Return {
                        value: retained.clone()
                    })
                    .unwrap(),
                    source
                );
                drop(retained);
                for invalid in [
                    format!("{{\"value\":{data},\"kind\":\"unknown\"}}"),
                    format!("{{\"value\":{data},\"kind\":\"return\",\"extra\":false}}"),
                    format!(
                        "{{\"value\":{data},\"kind\":\"domain_error\",\"reason_kind\":\"bad\"}}"
                    ),
                    format!("{{\"value\":{data},\"kind\":\"return\",\"value\":null}}"),
                ] {
                    let mut decoder = serde_json::Deserializer::from_str(&invalid);
                    decoder.disable_recursion_limit();
                    assert!(PortAnswer::deserialize(&mut decoder).is_err());
                }
                let source = format!("{{\"value\":{data},\"kind\":\"Return\"}}");
                let mut decoder = serde_json::Deserializer::from_str(&source);
                decoder.disable_recursion_limit();
                assert!(matches!(
                    HostAnswer::deserialize(&mut decoder).unwrap(),
                    HostAnswer::Return(_)
                ));
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
