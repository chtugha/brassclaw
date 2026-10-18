//! Shared JSON serialization mechanics. Callers own admission, resource policy
//! and writer bounds. Stack growth protects Serde callbacks only; it does not
//! make a value's Clone, Drop, Display or arbitrary application traversal safe.
use std::io::{self, Write};

use serde::Serialize;

/// Payload-free failure classification. A caller with its own bounded writer
/// can additionally inspect that writer's capacity flag after an encoding error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum JsonWriteError {
    #[error("JSON encoding failed")]
    Encoding,
    #[error("JSON byte limit exceeded")]
    LimitExceeded,
}

/// Preserve standard compact JSON encoding while protecting nested callbacks
/// with portable dynamic stack growth. This does not loosen any reader limit.
pub fn write_json<W: Write, T: Serialize + ?Sized>(
    writer: W,
    value: &T,
) -> Result<(), JsonWriteError> {
    let mut serializer = serde_json::Serializer::new(writer);
    value
        .serialize(serde_stacker::Serializer::new(&mut serializer))
        .map_err(|_| JsonWriteError::Encoding)
}

/// Count complete encoded bytes without allocating another full output copy.
/// A finite limit stops the writer; overflow never becomes partial success.
pub fn count_json_bytes<T: Serialize + ?Sized>(
    value: &T,
    limit: Option<u64>,
) -> Result<u64, JsonWriteError> {
    let mut counter = Counter {
        bytes: 0,
        limit,
        exceeded: false,
    };
    match write_json(&mut counter, value) {
        Ok(()) => Ok(counter.bytes),
        Err(_) if counter.exceeded => Err(JsonWriteError::LimitExceeded),
        Err(error) => Err(error),
    }
}

struct Counter {
    bytes: u64,
    limit: Option<u64>,
    exceeded: bool,
}

impl Write for Counter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let next = u64::try_from(buffer.len())
            .ok()
            .and_then(|bytes| self.bytes.checked_add(bytes))
            .filter(|bytes| self.limit.is_none_or(|limit| *bytes <= limit));
        let Some(next) = next else {
            self.exceeded = true;
            return Err(io::Error::other("JSON byte limit exceeded"));
        };
        self.bytes = next;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    #[test]
    fn writer_and_counter_preserve_compact_bytes_limits_and_error_classification() {
        let value = json!({"é":[null,true,-1,u64::MAX,1.0,"ß\u{0000}\\\""],"empty":{}});
        let expected = serde_json::to_vec(&value).unwrap();
        let mut actual = Vec::new();
        write_json(&mut actual, &value).unwrap();
        assert_eq!(actual, expected);
        let length = u64::try_from(actual.len()).unwrap();
        assert_eq!(count_json_bytes(&value, None).unwrap(), length);
        assert_eq!(count_json_bytes(&value, Some(length)).unwrap(), length);
        for maximum in [0, length - 1] {
            assert_eq!(
                count_json_bytes(&value, Some(maximum)),
                Err(JsonWriteError::LimitExceeded)
            );
        }
        struct Invalid;
        impl Serialize for Invalid {
            fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
                Err(serde::ser::Error::custom("private serializer payload"))
            }
        }
        let error = count_json_bytes(&Invalid, None).unwrap_err();
        assert_eq!(error, JsonWriteError::Encoding);
        assert!(!format!("{error:?} {error}").contains("private"));
    }

    #[test]
    fn nested_envelopes_and_late_writer_errors_are_stack_safe() {
        std::thread::Builder::new()
            .stack_size(64 * 1024)
            .spawn(|| {
                let depth = 12_000;
                let mut value = Value::Bool(true);
                for _ in 0..depth {
                    value = Value::Array(vec![value]);
                }
                #[derive(Serialize)]
                struct Envelope<'a> {
                    kind: &'a str,
                    value: &'a Value,
                }
                let envelope = Envelope {
                    kind: "return",
                    value: &value,
                };
                let mut actual = Vec::new();
                write_json(&mut actual, &envelope).unwrap();
                let expected = format!(
                    "{{\"kind\":\"return\",\"value\":{}true{}}}",
                    "[".repeat(depth),
                    "]".repeat(depth)
                );
                assert_eq!(actual, expected.as_bytes());
                let length = u64::try_from(actual.len()).unwrap();
                assert_eq!(count_json_bytes(&envelope, Some(length)).unwrap(), length);
                assert_eq!(
                    count_json_bytes(&envelope, Some(length - 1)),
                    Err(JsonWriteError::LimitExceeded)
                );
                // Serialization does not protect ownership operations. Dismantle
                // this test's deep chain explicitly rather than recursive Drop.
                loop {
                    match value {
                        Value::Array(mut values) => value = values.pop().unwrap(),
                        Value::Bool(true) => break,
                        _ => panic!("unexpected chain"),
                    }
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
