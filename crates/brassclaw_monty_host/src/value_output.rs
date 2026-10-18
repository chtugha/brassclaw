//! Bounded, version-pinned Monty graph traversal and custody of owned JSON.
//! Both construction and cleanup remain iterative, including late failures
//! after an earlier argument has already produced a deep subtree.
use std::collections::{BTreeMap, HashSet};

use monty_types::{
    CallArgs, ObjectRef,
    unstable::{MontyNode, child, node},
};
use num_traits::ToPrimitive;
use serde_json::Value;

use crate::{ValueBudget, VmBounds, VmError, VmFailure, identifier};

enum Frame<'a> {
    Visit(ObjectRef<'a>, usize),
    Array(usize),
    Object(Vec<String>),
}

fn reserve<T>(values: &mut Vec<T>, count: usize, failure: VmFailure) -> Result<(), VmError> {
    values
        .try_reserve(count)
        .map_err(|_| VmError::kind(failure))
}

fn string(value: &str, budget: &mut ValueBudget) -> Result<String, VmError> {
    budget.bytes(value.len())?;
    copy_string(value, budget.failure)
}

fn copy_string(value: &str, failure: VmFailure) -> Result<String, VmError> {
    let mut result = String::new();
    result
        .try_reserve_exact(value.len())
        .map_err(|_| VmError::kind(failure))?;
    result.push_str(value);
    Ok(result)
}

#[derive(Default)]
struct JsonForest {
    roots: Vec<Value>,
    visited: usize,
}

impl JsonForest {
    fn visit(&mut self, failure: VmFailure) -> Result<(), VmError> {
        let visited = self
            .visited
            .checked_add(1)
            .ok_or_else(|| VmError::kind(failure))?;
        // Reserve for all owned nodes, not only the current number of roots.
        // On error the same buffer dismantles their disjoint trees in Drop.
        let additional = visited
            .checked_sub(self.roots.len())
            .ok_or_else(|| VmError::kind(failure))?;
        reserve(&mut self.roots, additional, failure)?;
        self.visited = visited;
        Ok(())
    }
}

impl Drop for JsonForest {
    fn drop(&mut self) {
        // Every descendant was counted before construction. The reserved
        // capacity therefore also covers this worklist, without reallocating.
        while let Some(value) = self.roots.pop() {
            match value {
                Value::Array(values) => self.roots.extend(values),
                Value::Object(values) => self.roots.extend(values.into_values()),
                _ => {}
            }
        }
    }
}

fn admit_children(
    frames: &mut Vec<Frame<'_>>,
    count: usize,
    budget: &ValueBudget,
    queued: &mut usize,
) -> Result<(), VmError> {
    let remaining = budget
        .bounds
        .max_value_nodes
        .checked_sub(budget.nodes)
        .and_then(|remaining| remaining.checked_sub(*queued))
        .ok_or_else(|| VmError::kind(budget.failure))?;
    if count > remaining {
        return Err(VmError::kind(budget.failure));
    }
    reserve(
        frames,
        count
            .checked_add(1)
            .ok_or_else(|| VmError::kind(budget.failure))?,
        budget.failure,
    )?;
    *queued = queued
        .checked_add(count)
        .ok_or_else(|| VmError::kind(budget.failure))?;
    Ok(())
}

fn append(
    value: ObjectRef<'_>,
    budget: &mut ValueBudget,
    forest: &mut JsonForest,
) -> Result<(), VmError> {
    let failure = budget.failure;
    let mut frames = Vec::new();
    reserve(&mut frames, 1, failure)?;
    frames.push(Frame::Visit(value, 0));
    let mut queued = 1usize;
    while let Some(frame) = frames.pop() {
        let value = match frame {
            Frame::Array(count) => {
                let start = forest
                    .roots
                    .len()
                    .checked_sub(count)
                    .ok_or_else(|| VmError::kind(failure))?;
                let mut values = Vec::new();
                reserve(&mut values, count, failure)?;
                values.extend(forest.roots.drain(start..));
                Value::Array(values)
            }
            Frame::Object(keys) => {
                let start = forest
                    .roots
                    .len()
                    .checked_sub(keys.len())
                    .ok_or_else(|| VmError::kind(failure))?;
                let mut values = serde_json::Map::new();
                // Keys were validated before queuing children; no fallible
                // policy step remains while this local map owns their trees.
                for (key, value) in keys.into_iter().zip(forest.roots.drain(start..)) {
                    values.insert(key, value);
                }
                Value::Object(values)
            }
            Frame::Visit(value, depth) => {
                queued = queued
                    .checked_sub(1)
                    .ok_or_else(|| VmError::kind(failure))?;
                budget.node(depth)?;
                forest.visit(failure)?;
                match node(value) {
                    MontyNode::None => Value::Null,
                    MontyNode::Bool(value) => Value::Bool(*value),
                    MontyNode::String(value) => Value::String(string(value, budget)?),
                    MontyNode::Int(value) => Value::from(*value),
                    MontyNode::BigInt(value) => {
                        if let Some(value) = value.to_i64() {
                            Value::from(value)
                        } else {
                            Value::from(value.to_u64().ok_or_else(|| VmError::kind(failure))?)
                        }
                    }
                    MontyNode::Float(value) => Value::Number(
                        serde_json::Number::from_f64(*value)
                            .ok_or_else(|| VmError::kind(failure))?,
                    ),
                    MontyNode::List(values) => {
                        admit_children(&mut frames, values.len(), budget, &mut queued)?;
                        frames.push(Frame::Array(values.len()));
                        let depth = depth.checked_add(1).ok_or_else(|| VmError::kind(failure))?;
                        frames.extend(
                            values
                                .iter()
                                .rev()
                                .map(|id| Frame::Visit(child(value, *id), depth)),
                        );
                        continue;
                    }
                    MontyNode::Dict(values) => {
                        admit_children(&mut frames, values.len(), budget, &mut queued)?;
                        let mut keys = Vec::new();
                        reserve(&mut keys, values.len(), failure)?;
                        let mut seen = HashSet::new();
                        seen.try_reserve(values.len())
                            .map_err(|_| VmError::kind(failure))?;
                        for (key, _) in values {
                            let key = child(value, *key)
                                .as_str()
                                .ok_or_else(|| VmError::kind(failure))?;
                            if !seen.insert(key) {
                                return Err(VmError::kind(failure));
                            }
                            keys.push(string(key, budget)?);
                        }
                        frames.push(Frame::Object(keys));
                        let depth = depth.checked_add(1).ok_or_else(|| VmError::kind(failure))?;
                        frames.extend(
                            values
                                .iter()
                                .rev()
                                .map(|(_, id)| Frame::Visit(child(value, *id), depth)),
                        );
                        continue;
                    }
                    _ => return Err(VmError::kind(failure)),
                }
            }
        };
        forest.roots.push(value);
    }
    Ok(())
}

pub(super) fn result(value: ObjectRef<'_>, bounds: VmBounds) -> Result<Value, VmError> {
    let mut budget = ValueBudget::new(bounds, VmFailure::InvalidResult);
    let mut forest = JsonForest::default();
    append(value, &mut budget, &mut forest)?;
    if forest.roots.len() != 1 {
        return Err(VmError::kind(budget.failure));
    }
    forest
        .roots
        .pop()
        .ok_or_else(|| VmError::kind(budget.failure))
}

pub(super) fn arguments(
    args: &CallArgs,
    bounds: VmBounds,
) -> Result<(Vec<Value>, BTreeMap<String, Value>), VmError> {
    let failure = VmFailure::InvalidHostArguments;
    let mut budget = ValueBudget::new(bounds, failure);
    let mut forest = JsonForest::default();
    let count = args.args().len();
    for value in args.args() {
        append(value, &mut budget, &mut forest)?;
    }
    let mut names = Vec::new();
    let mut seen = HashSet::new();
    for (key, value) in args.kwargs() {
        let key = key
            .as_str()
            .filter(|name| identifier(name))
            .ok_or_else(|| VmError::kind(failure))?;
        budget.bytes(key.len())?;
        seen.try_reserve(1).map_err(|_| VmError::kind(failure))?;
        if !seen.insert(key) {
            return Err(VmError::kind(failure));
        }
        reserve(&mut names, 1, failure)?;
        // Its bytes were charged above; copy without charging them twice.
        names.push(copy_string(key, failure)?);
        append(value, &mut budget, &mut forest)?;
    }
    if count.checked_add(names.len()) != Some(forest.roots.len()) {
        return Err(VmError::kind(failure));
    }
    let mut positional = Vec::new();
    reserve(&mut positional, count, failure)?;
    // All validation and fallible reservations finish before custody transfers.
    positional.extend(forest.roots.drain(..count));
    let kwargs = names.into_iter().zip(forest.roots.drain(..)).collect();
    Ok((positional, kwargs))
}

#[cfg(test)]
mod tests {
    use super::*;
    use monty_types::{
        MontyObject,
        unstable::{MontyGraph, object_from_graph, object_from_node},
    };
    use serde_json::json;
    use std::time::Duration;

    fn bounds() -> VmBounds {
        VmBounds {
            max_source_bytes: 1024,
            max_compiled_source_bytes: 1024,
            max_feeds: 1,
            max_stdout_bytes: 1024,
            execution_slice: Duration::from_secs(1),
            max_value_depth: 16,
            max_value_nodes: 4096,
            max_value_bytes: 4096,
        }
    }

    fn deep(depth: usize) -> MontyObject {
        let mut graph = MontyGraph::new();
        let mut root = graph.push(MontyNode::Bool(true));
        for _ in 0..depth {
            root = graph.push(MontyNode::List(vec![root]));
        }
        object_from_graph(graph, root).unwrap()
    }

    #[test]
    fn output_preserves_values_and_aggregate_positional_keyword_budgets() {
        let input = json!({"empty":{},"array":[[],null,true,false,i64::MIN,u64::MAX,0.5,"ü\\\""]});
        let value = crate::json_input(&input, bounds()).unwrap();
        assert_eq!(result(value.as_ref(), bounds()).unwrap(), input);
        let mut call = CallArgs::new();
        call.push_arg(MontyObject::string("ß"));
        call.push_kwarg("mode", MontyObject::bool(true));
        let limits = VmBounds {
            max_value_nodes: 2,
            max_value_bytes: 6,
            ..bounds()
        };
        let (positional, keyword) = arguments(&call, limits).unwrap();
        assert_eq!(positional, [json!("ß")]);
        assert_eq!(keyword, BTreeMap::from([("mode".into(), json!(true))]));
        for limited in [
            VmBounds {
                max_value_nodes: 1,
                ..limits
            },
            VmBounds {
                max_value_bytes: 5,
                ..limits
            },
        ] {
            assert_eq!(
                arguments(&call, limited).unwrap_err().failure,
                VmFailure::InvalidHostArguments
            );
        }
        let keyed = crate::json_input(&json!({"é":["ß"]}), bounds()).unwrap();
        let limits = VmBounds {
            max_value_depth: 2,
            max_value_nodes: 3,
            max_value_bytes: 4,
            ..bounds()
        };
        assert_eq!(result(keyed.as_ref(), limits).unwrap(), json!({"é":["ß"]}));
        for limited in [
            VmBounds {
                max_value_depth: 1,
                ..limits
            },
            VmBounds {
                max_value_nodes: 2,
                ..limits
            },
            VmBounds {
                max_value_bytes: 3,
                ..limits
            },
        ] {
            assert_eq!(
                result(keyed.as_ref(), limited).unwrap_err().failure,
                VmFailure::InvalidResult
            );
        }
    }

    #[test]
    fn output_rejects_non_json_nodes_nonfinite_numbers_and_invalid_keys() {
        let duplicate = MontyObject::dict([
            (MontyObject::string("key"), MontyObject::int(1)),
            (MontyObject::string("key"), MontyObject::int(2)),
        ]);
        let numeric = MontyObject::dict([(MontyObject::int(1), MontyObject::int(2))]);
        for value in [
            duplicate,
            numeric,
            object_from_node(MontyNode::Tuple(vec![])),
            object_from_node(MontyNode::Bytes(vec![0])),
            object_from_node(MontyNode::Cycle("[...]".into())),
            MontyObject::float(f64::NAN),
            MontyObject::float(f64::INFINITY),
            MontyObject::bigint(u128::MAX.into()),
        ] {
            assert_eq!(
                result(value.as_ref(), bounds()).unwrap_err().failure,
                VmFailure::InvalidResult
            );
        }
    }

    #[test]
    fn shared_graph_expansion_is_charged_per_json_occurrence() {
        let depth = 8;
        let mut graph = MontyGraph::new();
        let mut root = graph.push(MontyNode::String("ß".into()));
        for _ in 0..depth {
            root = graph.push(MontyNode::List(vec![root, root]));
        }
        assert_eq!(graph.len(), depth + 1);
        let value = object_from_graph(graph, root).unwrap();
        let limits = VmBounds {
            max_value_depth: depth,
            max_value_nodes: 511,
            max_value_bytes: 512,
            ..bounds()
        };
        let output = result(value.as_ref(), limits).unwrap();
        let mut pending = vec![&output];
        let mut nodes = 0;
        let mut leaves = 0;
        while let Some(value) = pending.pop() {
            nodes += 1;
            match value {
                Value::Array(values) => pending.extend(values),
                Value::String(value) => {
                    assert_eq!(value, "ß");
                    leaves += 1;
                }
                _ => panic!("unexpected expanded value"),
            }
        }
        assert_eq!(nodes, 511);
        assert_eq!(leaves, 256);
        for limited in [
            VmBounds {
                max_value_nodes: 510,
                ..limits
            },
            VmBounds {
                max_value_bytes: 511,
                ..limits
            },
        ] {
            assert_eq!(
                result(value.as_ref(), limited).unwrap_err().failure,
                VmFailure::InvalidResult
            );
        }
    }

    #[test]
    fn deep_output_and_late_result_or_argument_failure_cleanup_use_a_small_stack() {
        std::thread::Builder::new()
            .stack_size(64 * 1024)
            .spawn(|| {
                let depth = 12_000;
                let value = deep(depth);
                let limits = VmBounds {
                    max_value_depth: depth + 2,
                    max_value_nodes: 2 * depth + 16,
                    max_value_bytes: 64,
                    ..bounds()
                };
                let mut output = result(value.as_ref(), limits).unwrap();
                for _ in 0..depth {
                    let Value::Array(mut values) = output else {
                        panic!("nested array required")
                    };
                    assert_eq!(values.len(), 1);
                    output = values.pop().unwrap();
                }
                assert_eq!(output, Value::Bool(true));
                let record = MontyObject::dict([(MontyObject::string("stored"), value.clone())]);
                for bad in [
                    object_from_node(MontyNode::Cycle("[...]".into())),
                    MontyObject::string("x".repeat(65)),
                ] {
                    let compound = MontyObject::list([record.clone(), bad]);
                    assert_eq!(
                        result(compound.as_ref(), limits).unwrap_err().failure,
                        VmFailure::InvalidResult
                    );
                }
                let mut call = CallArgs::new();
                call.push_arg(record.clone());
                call.push_arg(object_from_node(MontyNode::Cycle("[...]".into())));
                assert_eq!(
                    arguments(&call, limits).unwrap_err().failure,
                    VmFailure::InvalidHostArguments
                );

                let mut call = CallArgs::new();
                call.push_arg(record.clone());
                call.push_kwarg("ok", record.clone());
                call.push_kwarg("ok", MontyObject::bool(true));
                assert_eq!(
                    arguments(&call, limits).unwrap_err().failure,
                    VmFailure::InvalidHostArguments
                );

                let mut call = CallArgs::new();
                call.push_arg(record);
                call.push_kwarg("later_name_exceeds_budget", MontyObject::bool(true));
                assert_eq!(
                    arguments(
                        &call,
                        VmBounds {
                            max_value_bytes: 6,
                            ..limits
                        }
                    )
                    .unwrap_err()
                    .failure,
                    VmFailure::InvalidHostArguments
                );
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
