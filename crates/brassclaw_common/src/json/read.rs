//! Bounded JSON construction with iterative custody of partial trees.
//! No domain policy lives here: callers select limits and duplicate-key semantics.
//! Stack growth protects callbacks; the forest also protects late-error cleanup.
use std::{collections::BTreeMap, fmt};

use serde::de::{DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};

/// Independent bounds for one document. Object names count as nodes and bytes.
#[derive(Debug, Clone, Copy)]
pub struct JsonReadLimits {
    pub max_depth: usize,
    pub max_nodes: usize,
    pub max_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicateKeys {
    /// Component contracts reject ambiguous documents and sort object names.
    Reject,
    /// General JSON Tool compatibility: retain the last value and native map order.
    LastWins,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum JsonReadError {
    #[error("invalid JSON limits")]
    InvalidLimits,
    #[error("JSON document byte bound exceeded")]
    DocumentBytes,
    #[error("JSON value resource bound exceeded")]
    Resource,
    #[error("invalid JSON")]
    Syntax,
    #[error("duplicate JSON key")]
    DuplicateKey,
    #[error("trailing JSON data")]
    TrailingData,
}

/// Keeps owned values safe to discard even when a caller rejects deep data.
/// Taking the naked Value transfers responsibility for Clone/Drop/traversal.
pub struct JsonDocument(Forest);
impl fmt::Debug for JsonDocument {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JsonDocument")
            .finish_non_exhaustive()
    }
}
impl JsonDocument {
    pub fn as_value(&self) -> &Value {
        &self.0.roots[0]
    }
    pub fn into_value(mut self) -> Value {
        self.0.roots.pop().expect("completed JSON root")
    }
}

struct SizeBudget {
    limits: JsonReadLimits,
    nodes: usize,
    bytes: usize,
    failure: Option<JsonReadError>,
    duplicates: DuplicateKeys,
}
impl SizeBudget {
    fn new(limits: JsonReadLimits, duplicates: DuplicateKeys) -> Result<Self, JsonReadError> {
        if limits.max_depth == 0 || limits.max_nodes == 0 || limits.max_bytes == 0 {
            return Err(JsonReadError::InvalidLimits);
        }
        Ok(Self {
            limits,
            nodes: 0,
            bytes: 0,
            failure: None,
            duplicates,
        })
    }
    fn fail<E: serde::de::Error>(&mut self, error: JsonReadError) -> E {
        self.failure = Some(error);
        E::custom(error)
    }
    fn add_bytes<E: serde::de::Error>(&mut self, bytes: usize) -> Result<(), E> {
        match self
            .bytes
            .checked_add(bytes)
            .filter(|n| *n <= self.limits.max_bytes)
        {
            Some(bytes) => {
                self.bytes = bytes;
                Ok(())
            }
            None => Err(self.fail(JsonReadError::Resource)),
        }
    }
    fn enter<E: serde::de::Error>(&mut self, depth: usize, bytes: usize) -> Result<(), E> {
        match self
            .nodes
            .checked_add(1)
            .filter(|n| *n <= self.limits.max_nodes && depth <= self.limits.max_depth)
        {
            Some(nodes) => {
                self.nodes = nodes;
                self.add_bytes(bytes)
            }
            None => Err(self.fail(JsonReadError::Resource)),
        }
    }
}

#[derive(Default)]
struct Forest {
    roots: Vec<Value>,
    visited: usize,
}

impl Forest {
    fn enter<E: serde::de::Error>(&mut self) -> Result<usize, E> {
        self.visited = self
            .visited
            .checked_add(1)
            .ok_or_else(|| E::custom("capacity"))?;
        let additional = self
            .visited
            .checked_sub(self.roots.len())
            .ok_or_else(|| E::custom("capacity"))?;
        self.roots
            .try_reserve(additional)
            .map_err(|_| E::custom("capacity"))?;
        let index = self.roots.len();
        // A pending container's placeholder counts as that container's node.
        // Closing it removes its child roots before filling this slot.
        self.roots.push(Value::Null);
        Ok(index)
    }
}

impl Forest {
    fn discard_from(&mut self, start: usize) {
        // Owned descendants plus pending placeholders never exceed visited.
        // Reuse reserved capacity even after a late parse/budget failure.
        while self.roots.len() > start {
            let value = self.roots.pop().expect("owned root");
            match value {
                Value::Array(values) => self.roots.extend(values),
                Value::Object(values) => self.roots.extend(values.into_values()),
                _ => {}
            }
        }
    }
}

impl Drop for Forest {
    fn drop(&mut self) {
        self.discard_from(0);
    }
}

pub fn read_json(
    source: &str,
    limits: JsonReadLimits,
    duplicates: DuplicateKeys,
) -> Result<JsonDocument, JsonReadError> {
    let mut budget = SizeBudget::new(limits, duplicates)?;
    if source.len() > limits.max_bytes {
        return Err(JsonReadError::DocumentBytes);
    }
    let mut forest = Forest::default();
    let mut decoder = serde_json::Deserializer::from_str(source);
    // Only this bounded Seed may disable Serde's fixed recursion guard. Every
    // child is admitted before allocation and all owned error paths are guarded.
    decoder.disable_recursion_limit();
    Seed {
        budget: &mut budget,
        forest: &mut forest,
        depth: 0,
    }
    .deserialize(serde_stacker::Deserializer::new(&mut decoder))
    .map_err(|_| budget.failure.unwrap_or(JsonReadError::Syntax))?;
    decoder.end().map_err(|_| JsonReadError::TrailingData)?;
    Ok(JsonDocument(forest))
}

/// Bounded clone without serializing an extra document or recursively cloning
/// untrusted values. Failures keep partial descendants under iterative custody.
pub fn clone_json(value: &Value, limits: JsonReadLimits) -> Result<JsonDocument, JsonReadError> {
    let mut budget = SizeBudget::new(limits, DuplicateKeys::LastWins)?;
    let mut forest = Forest::default();
    Seed {
        budget: &mut budget,
        forest: &mut forest,
        depth: 0,
    }
    .deserialize(serde_stacker::Deserializer::new(value))
    .map_err(|_| budget.failure.unwrap_or(JsonReadError::Syntax))?;
    Ok(JsonDocument(forest))
}

/// Check already owned input before copying/formatting it. Traversal uses only
/// borrowed frames; rejecting a value never owns or recursively drops it.
pub fn check_json_value(value: &Value, limits: JsonReadLimits) -> Result<(), JsonReadError> {
    enum Frame<'a> {
        Value(&'a Value, usize),
        Array(std::slice::Iter<'a, Value>, usize),
        Object(serde_json::map::Iter<'a>, usize),
    }
    let mut budget = SizeBudget::new(limits, DuplicateKeys::LastWins)?;
    let mut frames = vec![Frame::Value(value, 0)];
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Value(value, depth) => {
                budget
                    .enter::<serde_json::Error>(depth, value.as_str().map_or(0, str::len))
                    .map_err(|_| JsonReadError::Resource)?;
                match value {
                    Value::Array(values) => frames.push(Frame::Array(values.iter(), depth + 1)),
                    Value::Object(values) => frames.push(Frame::Object(values.iter(), depth + 1)),
                    _ => {}
                }
            }
            Frame::Array(mut values, depth) => {
                if let Some(value) = values.next() {
                    frames.push(Frame::Array(values, depth));
                    frames.push(Frame::Value(value, depth));
                }
            }
            Frame::Object(mut values, depth) => {
                if let Some((key, value)) = values.next() {
                    budget
                        .enter::<serde_json::Error>(depth, key.len())
                        .map_err(|_| JsonReadError::Resource)?;
                    frames.push(Frame::Object(values, depth));
                    frames.push(Frame::Value(value, depth));
                }
            }
        }
    }
    Ok(())
}

struct Seed<'a> {
    budget: &'a mut SizeBudget,
    forest: &'a mut Forest,
    depth: usize,
}

impl<'de> DeserializeSeed<'de> for Seed<'_> {
    type Value = ();

    fn deserialize<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        self.budget.enter(self.depth, 0)?;
        let index = self
            .forest
            .enter::<D::Error>()
            .map_err(|_| self.budget.fail::<D::Error>(JsonReadError::Resource))?;
        deserializer.deserialize_any(JsonVisitor {
            budget: self.budget,
            forest: self.forest,
            depth: self.depth,
            index,
        })
    }
}

struct JsonVisitor<'a> {
    budget: &'a mut SizeBudget,
    forest: &'a mut Forest,
    depth: usize,
    index: usize,
}

impl<'de> Visitor<'de> for JsonVisitor<'_> {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("bounded JSON data")
    }

    fn visit_unit<E: serde::de::Error>(self) -> Result<(), E> {
        Ok(())
    }

    fn visit_bool<E: serde::de::Error>(self, value: bool) -> Result<(), E> {
        self.forest.roots[self.index] = Value::Bool(value);
        Ok(())
    }

    fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<(), E> {
        self.forest.roots[self.index] = Value::Number(value.into());
        Ok(())
    }

    fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<(), E> {
        self.forest.roots[self.index] = Value::Number(value.into());
        Ok(())
    }

    fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<(), E> {
        let number = Number::from_f64(value).ok_or_else(|| E::custom("non-finite number"))?;
        self.forest.roots[self.index] = Value::Number(number);
        Ok(())
    }

    fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<(), E> {
        self.budget.add_bytes(value.len())?;
        let mut owned = String::new();
        owned
            .try_reserve_exact(value.len())
            .map_err(|_| self.budget.fail::<E>(JsonReadError::Resource))?;
        owned.push_str(value);
        self.forest.roots[self.index] = Value::String(owned);
        Ok(())
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<(), A::Error> {
        while sequence
            .next_element_seed(Seed {
                budget: self.budget,
                forest: self.forest,
                depth: self.depth + 1,
            })?
            .is_some()
        {}
        let start = self.index + 1;
        let count = self.forest.roots.len() - start;
        let mut values = Vec::new();
        values
            .try_reserve(count)
            .map_err(|_| self.budget.fail::<A::Error>(JsonReadError::Resource))?;
        values.extend(self.forest.roots.drain(start..));
        self.forest.roots[self.index] = Value::Array(values);
        Ok(())
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        let mut sorted = BTreeMap::new();
        let mut native = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            self.budget.enter(self.depth + 1, key.len())?;
            if self.budget.duplicates == DuplicateKeys::Reject && sorted.contains_key(&key) {
                return Err(self.budget.fail(JsonReadError::DuplicateKey));
            }
            let index = self.forest.roots.len();
            map.next_value_seed(Seed {
                budget: self.budget,
                forest: self.forest,
                depth: self.depth + 1,
            })?;
            match self.budget.duplicates {
                DuplicateKeys::Reject => {
                    sorted.insert(key, index);
                }
                DuplicateKeys::LastWins => {
                    native.insert(key, Value::from(index));
                }
            }
        }
        let mut values = Map::new();
        // No fallible operation remains while the local map owns descendants.
        for (key, index) in sorted {
            values.insert(key, std::mem::take(&mut self.forest.roots[index]));
        }
        for (key, index) in native {
            let index = usize::try_from(index.as_u64().expect("internal root index"))
                .expect("original native root index");
            values.insert(key, std::mem::take(&mut self.forest.roots[index]));
        }
        // Overwritten duplicate values may still own arbitrarily deep trees.
        // Never use truncate/clear here: they recursively drop those values.
        self.forest.discard_from(self.index + 1);
        self.forest.roots[self.index] = Value::Object(values);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reader_preserves_native_values_and_distinguishes_syntax_from_resource_failure() {
        let limits = JsonReadLimits {
            max_depth: 8,
            max_nodes: 128,
            max_bytes: 4096,
        };
        let source =
            r#"{"z":[null,true,-1,18446744073709551615,1.0,"é\u0000"],"a":{},"z":["last"]}"#;
        let native: Value = serde_json::from_str(source).unwrap();
        let read = read_json(source, limits, DuplicateKeys::LastWins).unwrap();
        assert_eq!(read.as_value(), &native);
        assert_eq!(
            clone_json(read.as_value(), limits).unwrap().as_value(),
            &native
        );
        assert_eq!(
            read_json(source, limits, DuplicateKeys::Reject).unwrap_err(),
            JsonReadError::DuplicateKey
        );
        for invalid in ["[1,]", "1e1000", r#""\uD800""#] {
            assert_eq!(
                read_json(invalid, limits, DuplicateKeys::LastWins).unwrap_err(),
                JsonReadError::Syntax
            );
        }
        assert_eq!(
            read_json("true false", limits, DuplicateKeys::LastWins).unwrap_err(),
            JsonReadError::TrailingData
        );
        for (source, nodes) in [
            ("[]", 1),
            ("[null]", 2),
            (r#"{"x":0}"#, 3),
            (r#"{"x":0,"x":1}"#, 5),
        ] {
            let exact = JsonReadLimits {
                max_nodes: nodes,
                ..limits
            };
            assert!(read_json(source, exact, DuplicateKeys::LastWins).is_ok());
            if nodes > 1 {
                assert_eq!(
                    read_json(
                        source,
                        JsonReadLimits {
                            max_nodes: nodes - 1,
                            ..limits
                        },
                        DuplicateKeys::LastWins
                    )
                    .unwrap_err(),
                    JsonReadError::Resource
                );
            }
        }
        assert_eq!(
            read_json(
                "true",
                JsonReadLimits {
                    max_bytes: 3,
                    ..limits
                },
                DuplicateKeys::LastWins
            )
            .unwrap_err(),
            JsonReadError::DocumentBytes
        );
        assert_eq!(
            read_json(
                "[[true]]",
                JsonReadLimits {
                    max_depth: 1,
                    ..limits
                },
                DuplicateKeys::LastWins
            )
            .unwrap_err(),
            JsonReadError::Resource
        );
    }

    #[test]
    fn deep_success_duplicates_clone_and_late_failures_keep_iterative_custody() {
        std::thread::Builder::new()
            .stack_size(64 * 1024)
            .spawn(|| {
                let depth = 12_000;
                let limits = JsonReadLimits {
                    max_depth: depth + 4,
                    max_nodes: depth + 32,
                    max_bytes: depth * 3,
                };
                let branch = format!("{}true{}", "[".repeat(depth), "]".repeat(depth));
                let doc = read_json(&branch, limits, DuplicateKeys::LastWins).unwrap();
                check_json_value(doc.as_value(), limits).unwrap();
                drop(clone_json(doc.as_value(), limits).unwrap());
                assert_eq!(
                    clone_json(
                        doc.as_value(),
                        JsonReadLimits {
                            max_nodes: depth,
                            ..limits
                        }
                    )
                    .unwrap_err(),
                    JsonReadError::Resource
                );
                // The overwritten branch must be dismantled, not recursively dropped.
                let duplicate = format!("{{\"x\":{branch},\"x\":false}}");
                assert_eq!(
                    read_json(&duplicate, limits, DuplicateKeys::LastWins)
                        .unwrap()
                        .as_value()["x"],
                    false
                );
                assert_eq!(
                    read_json(&duplicate, limits, DuplicateKeys::Reject).unwrap_err(),
                    JsonReadError::DuplicateKey
                );
                for invalid in [
                    format!("[{branch},]"),
                    format!("{{\"x\":{branch},\"later\":}}"),
                    format!("{branch} private trailing data"),
                ] {
                    let error = read_json(&invalid, limits, DuplicateKeys::LastWins).unwrap_err();
                    assert!(!format!("{error:?} {error}").contains("private"));
                }
                drop(doc);
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
