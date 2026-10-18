//! Custody of copied dispatch output through completion checks and auditing.
//! Borrow the source; never recursively clone or drop an intermediate tree.
use brassclaw_capabilities::CapabilityObligationError;
use brassclaw_safety::LeakDetector;
use serde_json::Value;

use super::output_obligation_failed;

enum Frame<'a> {
    Visit(&'a Value),
    Array(usize),
    Object(Vec<String>),
}

#[derive(Default)]
pub(super) struct Output {
    roots: Vec<Value>,
    visited: usize,
}

impl Output {
    fn visit(&mut self) -> Result<(), CapabilityObligationError> {
        self.visited = self
            .visited
            .checked_add(1)
            .ok_or_else(output_obligation_failed)?;
        let additional = self
            .visited
            .checked_sub(self.roots.len())
            .ok_or_else(output_obligation_failed)?;
        reserve(&mut self.roots, additional)
    }

    pub(super) fn value(&self) -> &Value {
        self.roots.last().expect("completed output traversal")
    }

    pub(super) fn into_value(mut self) -> Value {
        self.roots.pop().expect("completed output traversal")
    }
}

impl Drop for Output {
    fn drop(&mut self) {
        // Construction reserves for every visited node. These disjoint trees
        // contain at most that many nodes, so cleanup needs no new allocation.
        while let Some(value) = self.roots.pop() {
            match value {
                Value::Array(values) => self.roots.extend(values),
                Value::Object(values) => self.roots.extend(values.into_values()),
                _ => {}
            }
        }
    }
}

fn reserve<T>(values: &mut Vec<T>, count: usize) -> Result<(), CapabilityObligationError> {
    values
        .try_reserve(count)
        .map_err(|_| output_obligation_failed())
}

fn string(
    value: &str,
    detector: Option<&LeakDetector>,
) -> Result<String, CapabilityObligationError> {
    if let Some(detector) = detector {
        return detector
            .scan_and_clean(value)
            .map_err(|_| output_obligation_failed());
    }
    let mut copy = String::new();
    copy.try_reserve_exact(value.len())
        .map_err(|_| output_obligation_failed())?;
    copy.push_str(value);
    Ok(copy)
}

pub(super) fn copy_output(
    source: &Value,
    detector: Option<&LeakDetector>,
) -> Result<Output, CapabilityObligationError> {
    let mut output = Output::default();
    let mut frames = Vec::new();
    reserve(&mut frames, 1)?;
    frames.push(Frame::Visit(source));
    while let Some(frame) = frames.pop() {
        let value = match frame {
            Frame::Array(count) => {
                let start = output
                    .roots
                    .len()
                    .checked_sub(count)
                    .ok_or_else(output_obligation_failed)?;
                let mut values = Vec::new();
                reserve(&mut values, count)?;
                values.extend(output.roots.drain(start..));
                Value::Array(values)
            }
            Frame::Object(keys) => {
                let start = output
                    .roots
                    .len()
                    .checked_sub(keys.len())
                    .ok_or_else(output_obligation_failed)?;
                let mut values = serde_json::Map::new();
                // Key validation and all fallible traversal work precede
                // transferring deep children out of the cleanup guard.
                for (key, value) in keys.into_iter().zip(output.roots.drain(start..)) {
                    values.insert(key, value);
                }
                Value::Object(values)
            }
            Frame::Visit(value) => {
                output.visit()?;
                match value {
                    Value::Null => Value::Null,
                    Value::Bool(value) => Value::Bool(*value),
                    Value::Number(value) => Value::Number(value.clone()),
                    Value::String(value) => Value::String(string(value, detector)?),
                    Value::Array(values) => {
                        reserve(
                            &mut frames,
                            values
                                .len()
                                .checked_add(1)
                                .ok_or_else(output_obligation_failed)?,
                        )?;
                        frames.push(Frame::Array(values.len()));
                        frames.extend(values.iter().rev().map(Frame::Visit));
                        continue;
                    }
                    Value::Object(values) => {
                        reserve(
                            &mut frames,
                            values
                                .len()
                                .checked_add(1)
                                .ok_or_else(output_obligation_failed)?,
                        )?;
                        let mut keys = Vec::new();
                        reserve(&mut keys, values.len())?;
                        for key in values.keys() {
                            keys.push(string(key, detector)?);
                        }
                        if detector.is_some() {
                            // Redaction can make distinct keys identical.
                            // Sort references, preserving key/value alignment
                            // without another string copy or a quadratic scan.
                            let mut ordered = Vec::new();
                            reserve(&mut ordered, keys.len())?;
                            ordered.extend(keys.iter().map(String::as_str));
                            ordered.sort_unstable();
                            if ordered.windows(2).any(|pair| pair[0] == pair[1]) {
                                return Err(output_obligation_failed());
                            }
                        }
                        frames.push(Frame::Object(keys));
                        frames.extend(values.values().rev().map(Frame::Visit));
                        continue;
                    }
                }
            }
        };
        output.roots.push(value);
    }
    Ok(output)
}
