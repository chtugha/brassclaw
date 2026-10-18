//! Version-pinned JSON input adapter. Build one post-order Monty arena without
//! recursive Rust calls or copying each child's graph into every ancestor.
use monty_types::{
    MontyObject,
    unstable::{MontyGraph, MontyNode, NodeId, object_from_graph},
};
use serde_json::Value;

use crate::{ValueBudget, VmBounds, VmError, VmFailure};

enum Frame<'a> {
    Visit(&'a Value, usize),
    Array(usize),
    Object(Vec<NodeId>),
}

fn invalid() -> VmError {
    VmError::kind(VmFailure::InvalidInputs)
}

fn reserve<T>(values: &mut Vec<T>, count: usize) -> Result<(), VmError> {
    values.try_reserve(count).map_err(|_| invalid())
}

fn string(value: &str) -> Result<String, VmError> {
    let mut result = String::new();
    result
        .try_reserve_exact(value.len())
        .map_err(|_| invalid())?;
    result.push_str(value);
    Ok(result)
}

fn push(nodes: &mut Vec<MontyNode>, node: MontyNode) -> Result<NodeId, VmError> {
    let id = NodeId(u32::try_from(nodes.len()).map_err(|_| invalid())?);
    reserve(nodes, 1)?;
    nodes.push(node);
    Ok(id)
}

fn children(roots: &mut Vec<NodeId>, count: usize) -> Result<Vec<NodeId>, VmError> {
    let start = roots.len().checked_sub(count).ok_or_else(invalid)?;
    let mut children = Vec::new();
    reserve(&mut children, count)?;
    children.extend(roots.drain(start..));
    Ok(children)
}

fn admit_children(
    frames: &mut Vec<Frame<'_>>,
    count: usize,
    budget: &ValueBudget,
    queued: &mut usize,
) -> Result<(), VmError> {
    // Every child costs at least one JSON value node. Reject an oversized
    // container before reserving its traversal slots or graph references.
    let remaining = budget
        .bounds
        .max_value_nodes
        .checked_sub(budget.nodes)
        .and_then(|remaining| remaining.checked_sub(*queued))
        .ok_or_else(invalid)?;
    if count > remaining {
        return Err(invalid());
    }
    reserve(frames, count.checked_add(1).ok_or_else(invalid)?)?;
    *queued = queued.checked_add(count).ok_or_else(invalid)?;
    Ok(())
}

pub(super) fn convert(value: &Value, bounds: VmBounds) -> Result<MontyObject, VmError> {
    let mut budget = ValueBudget::new(bounds, VmFailure::InvalidInputs);
    let mut frames = Vec::new();
    reserve(&mut frames, 1)?;
    frames.push(Frame::Visit(value, 0));
    let mut queued = 1usize;
    let mut nodes = Vec::new();
    let mut roots = Vec::new();
    while let Some(frame) = frames.pop() {
        let node = match frame {
            Frame::Array(count) => MontyNode::List(children(&mut roots, count)?),
            Frame::Object(keys) => {
                let values = children(&mut roots, keys.len())?;
                let mut pairs = Vec::new();
                reserve(&mut pairs, keys.len())?;
                pairs.extend(keys.into_iter().zip(values));
                MontyNode::Dict(pairs)
            }
            Frame::Visit(value, depth) => {
                queued = queued.checked_sub(1).ok_or_else(invalid)?;
                budget.node(depth)?;
                match value {
                    Value::Null => MontyNode::None,
                    Value::Bool(value) => MontyNode::Bool(*value),
                    Value::Number(value) => {
                        if let Some(value) = value.as_i64() {
                            MontyNode::Int(value)
                        } else if let Some(value) = value.as_u64() {
                            MontyNode::BigInt(value.into())
                        } else {
                            MontyNode::Float(value.as_f64().ok_or_else(invalid)?)
                        }
                    }
                    Value::String(value) => {
                        budget.bytes(value.len())?;
                        MontyNode::String(string(value)?)
                    }
                    Value::Array(values) => {
                        admit_children(&mut frames, values.len(), &budget, &mut queued)?;
                        frames.push(Frame::Array(values.len()));
                        let depth = depth.checked_add(1).ok_or_else(invalid)?;
                        frames.extend(values.iter().rev().map(|value| Frame::Visit(value, depth)));
                        continue;
                    }
                    Value::Object(values) => {
                        admit_children(&mut frames, values.len(), &budget, &mut queued)?;
                        let mut keys = Vec::new();
                        reserve(&mut keys, values.len())?;
                        for key in values.keys() {
                            budget.bytes(key.len())?;
                            keys.push(push(&mut nodes, MontyNode::String(string(key)?))?);
                        }
                        frames.push(Frame::Object(keys));
                        let depth = depth.checked_add(1).ok_or_else(invalid)?;
                        frames.extend(
                            values
                                .values()
                                .rev()
                                .map(|value| Frame::Visit(value, depth)),
                        );
                        continue;
                    }
                }
            }
        };
        reserve(&mut roots, 1)?;
        roots.push(push(&mut nodes, node)?);
    }
    let [root] = roots.as_slice() else {
        return Err(invalid());
    };
    let graph = MontyGraph::from_nodes(nodes).map_err(|_| invalid())?;
    object_from_graph(graph, *root).map_err(|_| invalid())
}

#[cfg(test)]
mod tests {
    use super::*;
    use monty_types::unstable::graph_parts;
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
            max_value_nodes: 1024,
            max_value_bytes: 1024,
        }
    }

    #[test]
    fn input_arena_preserves_json_values_and_exact_budget_accounting() {
        let value = json!({"empty":{}, "array":[[],null,true,false,i64::MIN,u64::MAX,0.5,"ü\\\""]});
        let converted = convert(&value, bounds()).unwrap();
        assert_eq!(
            crate::json_output(converted.as_ref(), bounds()).unwrap(),
            value
        );
        let value = json!({"é":["ß"]});
        let mut limits = bounds();
        limits.max_value_depth = 2;
        limits.max_value_nodes = 3;
        limits.max_value_bytes = 4;
        let converted = convert(&value, limits).unwrap();
        assert_eq!(graph_parts(&converted).0.len(), 4); // keys cost bytes, not value nodes
        assert_eq!(
            crate::json_output(converted.as_ref(), limits).unwrap(),
            value
        );
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
                convert(&value, limited).unwrap_err().failure,
                VmFailure::InvalidInputs
            );
        }
    }

    #[test]
    fn deep_input_construction_and_rejection_use_a_flat_arena_on_a_small_stack() {
        std::thread::Builder::new()
            .stack_size(64 * 1024)
            .spawn(|| {
                let depth = 12_000;
                let mut value = Value::Bool(true);
                for _ in 0..depth {
                    value = Value::Array(vec![value]);
                }
                let limits = VmBounds {
                    max_value_depth: depth,
                    max_value_nodes: depth + 1,
                    ..bounds()
                };
                let converted = convert(&value, limits).unwrap();
                let (graph, root) = graph_parts(&converted);
                assert_eq!(graph.len(), depth + 1);
                assert_eq!(root.index(), depth);
                assert_eq!(graph.node(NodeId(0)), &MontyNode::Bool(true));
                for (index, node) in graph.nodes().iter().enumerate().skip(1) {
                    let MontyNode::List(children) = node else {
                        panic!("array node required")
                    };
                    assert_eq!(
                        children.as_slice(),
                        &[NodeId(u32::try_from(index - 1).unwrap())]
                    );
                }
                assert_eq!(
                    convert(
                        &value,
                        VmBounds {
                            max_value_depth: depth - 1,
                            ..limits
                        }
                    )
                    .unwrap_err()
                    .failure,
                    VmFailure::InvalidInputs
                );
                drop(converted); // Monty's graph is flat, including cleanup.
                // serde_json::Value still has recursive Drop. Do not mistake this
                // adapter test for accepting deep values across the complete host.
                loop {
                    match value {
                        Value::Array(mut values) => value = values.pop().unwrap(),
                        Value::Bool(true) => break,
                        _ => panic!("unexpected chain value"),
                    }
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
