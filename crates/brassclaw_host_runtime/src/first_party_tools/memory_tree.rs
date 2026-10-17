//! One bounded Memory listing. Workflow sequencing remains in Recipes.
use std::collections::{BTreeMap, BTreeSet};

use brassclaw_host_api::{MontyExecutionLimits, RuntimeDispatchErrorKind};
use serde_json::{Map, Value};

use super::{HostPrimitiveSettings, HostPrimitiveSettingsSource, input_error};
use crate::FirstPartyCapabilityError;

fn capacity_error() -> FirstPartyCapabilityError {
    FirstPartyCapabilityError::new(RuntimeDispatchErrorKind::Resource)
}

pub(super) struct TreeRequest {
    root: String,
    depth: usize,
    limits: Limits,
}
#[derive(Clone, Copy)]
struct Limits {
    depth: usize,
    nodes: usize,
    bytes: usize,
}
impl TreeRequest {
    pub(super) async fn capture(
        input: &Value,
        source: Option<&dyn HostPrimitiveSettingsSource>,
    ) -> Result<Self, FirstPartyCapabilityError> {
        let input = input.as_object().ok_or_else(input_error)?;
        if input
            .keys()
            .any(|key| !matches!(key.as_str(), "path" | "depth"))
        {
            return Err(input_error());
        }
        let settings = match source {
            Some(source) => source.snapshot().await?,
            // Direct host/test consumers have canonical defaults. Production
            // supplies a required source bound before ingress is enabled.
            None => HostPrimitiveSettings {
                revision: 0,
                limits: MontyExecutionLimits::default(),
            },
        };
        settings.limits.validate().map_err(|_| capacity_error())?;
        let root = match input.get("path") {
            None => "",
            Some(value) => value.as_str().ok_or_else(input_error)?,
        };
        super::memory::reject_local_or_traversal_path(root)?;
        let depth = match input.get("depth") {
            None => settings.limits.default_memory_tree_depth,
            Some(value) => value
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .ok_or_else(input_error)?,
        };
        if depth == 0 || depth > settings.limits.max_memory_tree_depth {
            return Err(input_error());
        }
        Ok(Self {
            root: root.trim_matches('/').to_owned(),
            depth: usize::try_from(depth).map_err(|_| capacity_error())?,
            limits: Limits {
                depth: usize::try_from(settings.limits.max_value_depth)
                    .map_err(|_| capacity_error())?,
                nodes: usize::try_from(settings.limits.max_value_nodes)
                    .map_err(|_| capacity_error())?,
                bytes: usize::try_from(settings.limits.max_value_bytes)
                    .map_err(|_| capacity_error())?,
            },
        })
    }

    pub(super) fn build(&self, paths: &[String]) -> Result<Value, FirstPartyCapabilityError> {
        let prefix = if self.root.is_empty() {
            String::new()
        } else {
            format!("{}/", self.root)
        };
        // Flat arena: even a deep rejected path never creates recursively
        // owned JSON. Every child index is greater than its parent's index.
        let mut arena = vec![Directory::default()];
        let mut items = 0_usize;
        let mut retained_bytes = 0_usize;
        for path in paths {
            let Some(relative) = path.strip_prefix(&prefix).filter(|path| !path.is_empty()) else {
                continue;
            };
            let mut parts = relative.split('/').peekable();
            let mut parent = 0;
            let mut level = 1;
            while let Some(part) = parts.next() {
                let directory = parts.peek().is_some();
                let fresh = if directory {
                    !arena[parent].directories.contains_key(part)
                } else {
                    !arena[parent].files.contains(part)
                };
                if fresh {
                    items = items.checked_add(1).ok_or_else(capacity_error)?;
                    retained_bytes = retained_bytes
                        .checked_add(part.len())
                        .and_then(|n| n.checked_add(2 + usize::from(directory)))
                        .ok_or_else(capacity_error)?;
                    // Necessary lower bounds, not conservative rejection of
                    // otherwise valid output. Exact statistics follow below.
                    if items >= self.limits.nodes || retained_bytes > self.limits.bytes {
                        return Err(capacity_error());
                    }
                    if directory {
                        let child = if level < self.depth {
                            arena.try_reserve(1).map_err(|_| capacity_error())?;
                            let index = arena.len();
                            arena.push(Directory::default());
                            Some(index)
                        } else {
                            None
                        };
                        arena[parent].directories.insert(part.to_owned(), child);
                    } else {
                        arena[parent].files.insert(part.to_owned());
                    }
                }
                if !directory || level == self.depth {
                    break;
                }
                parent = arena[parent]
                    .directories
                    .get(part)
                    .copied()
                    .flatten()
                    .ok_or_else(capacity_error)?;
                level += 1;
            }
        }
        let mut statistics = Vec::new();
        statistics
            .try_reserve_exact(arena.len())
            .map_err(|_| capacity_error())?;
        statistics.resize(arena.len(), Statistics::default());
        for (index, directory) in arena.iter().enumerate().rev() {
            let mut total = Statistics {
                nodes: 1,
                bytes: 2,
                depth: 0,
            };
            let count = directory.directories.len() + directory.files.len();
            total.add_bytes(count.saturating_sub(1), self.limits)?;
            for (name, child) in &directory.directories {
                let name = format!("{name}/");
                let value = match child {
                    Some(child) => {
                        let mut child = statistics[*child];
                        child.add_bytes(
                            string_bytes(&name)?
                                .checked_add(3)
                                .ok_or_else(capacity_error)?,
                            self.limits,
                        )?;
                        child.nodes = child.nodes.checked_add(1).ok_or_else(capacity_error)?;
                        child.depth = child.depth.checked_add(1).ok_or_else(capacity_error)?;
                        child
                    }
                    None => Statistics::string(&name)?,
                };
                total.add(value, self.limits)?;
            }
            for name in &directory.files {
                total.add(Statistics::string(name)?, self.limits)?;
            }
            total.check(self.limits)?;
            statistics[index] = total;
        }
        // Only now materialize nested Values. All actual nodes, encoded bytes
        // and depth were checked before recursive ownership/serialization.
        let mut values: Vec<Option<Value>> = Vec::new();
        values
            .try_reserve_exact(arena.len())
            .map_err(|_| capacity_error())?;
        values.resize_with(arena.len(), || None);
        for (index, directory) in arena.into_iter().enumerate().rev() {
            let mut children = Vec::new();
            children
                .try_reserve_exact(directory.directories.len() + directory.files.len())
                .map_err(|_| capacity_error())?;
            for (name, child) in directory.directories {
                let name = format!("{name}/");
                children.push(match child {
                    Some(child) => Value::Object(Map::from_iter([(
                        name,
                        values[child].take().ok_or_else(capacity_error)?,
                    )])),
                    None => Value::String(name),
                });
            }
            children.extend(directory.files.into_iter().map(Value::String));
            values[index] = Some(Value::Array(children));
        }
        values[0].take().ok_or_else(capacity_error)
    }
}
#[derive(Default)]
struct Directory {
    directories: BTreeMap<String, Option<usize>>,
    files: BTreeSet<String>,
}
#[derive(Clone, Copy, Default)]
struct Statistics {
    nodes: usize,
    bytes: usize,
    depth: usize,
}
impl Statistics {
    fn string(value: &str) -> Result<Self, FirstPartyCapabilityError> {
        Ok(Self {
            nodes: 1,
            bytes: string_bytes(value)?,
            depth: 0,
        })
    }
    fn add_bytes(&mut self, bytes: usize, limits: Limits) -> Result<(), FirstPartyCapabilityError> {
        self.bytes = self.bytes.checked_add(bytes).ok_or_else(capacity_error)?;
        self.check(limits)
    }
    fn add(&mut self, child: Self, limits: Limits) -> Result<(), FirstPartyCapabilityError> {
        self.nodes = self
            .nodes
            .checked_add(child.nodes)
            .ok_or_else(capacity_error)?;
        self.bytes = self
            .bytes
            .checked_add(child.bytes)
            .ok_or_else(capacity_error)?;
        self.depth = self
            .depth
            .max(child.depth.checked_add(1).ok_or_else(capacity_error)?);
        self.check(limits)
    }
    fn check(self, limits: Limits) -> Result<(), FirstPartyCapabilityError> {
        if self.nodes > limits.nodes || self.bytes > limits.bytes || self.depth > limits.depth {
            Err(capacity_error())
        } else {
            Ok(())
        }
    }
}
fn string_bytes(value: &str) -> Result<usize, FirstPartyCapabilityError> {
    value.bytes().try_fold(2_usize, |bytes, byte| {
        let encoded = match byte {
            b'"' | b'\\' | 8..=10 | 12..=13 => 2,
            0..=31 => 6,
            _ => 1,
        };
        bytes.checked_add(encoded).ok_or_else(capacity_error)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn tree_optional_values_are_strict_and_the_legacy_maximum_is_not_a_clamp() {
        for input in [
            json!({"depth":0}),
            json!({"depth":11}),
            json!({"depth":null}),
            json!({"depth":"null"}),
            json!({"depth":-1}),
            json!({"depth":1.5}),
            json!({"depth":true}),
            json!({"depth":4294967296_u64}),
            json!({"path":null}),
            json!({"path":42}),
            json!([]),
            json!({"foreign":1}),
        ] {
            assert!(TreeRequest::capture(&input, None).await.is_err(), "{input}");
        }
        let tree = TreeRequest::capture(&json!({}), None).await.unwrap();
        assert_eq!(
            tree.build(&["a/b.md".into(), "z.md".into()]).unwrap(),
            json!(["a/", "z.md"])
        );
    }

    #[test]
    fn iterative_tree_preserves_order_and_checks_exact_encoded_result_bounds() {
        let paths = ["z.md".into(), "a/y.md".into(), "a/x/n.md".into()];
        let mut request = TreeRequest {
            root: String::new(),
            depth: 2,
            limits: Limits {
                depth: 64,
                nodes: 100,
                bytes: 1000,
            },
        };
        let result = request.build(&paths).unwrap();
        assert_eq!(result, json!([{"a/":["x/","y.md"]},"z.md"]));
        let bytes = serde_json::to_vec(&result).unwrap().len();
        request.limits = Limits {
            depth: 3,
            nodes: 6,
            bytes,
        };
        assert_eq!(request.build(&paths).unwrap(), result);
        for limits in [
            Limits {
                nodes: 5,
                ..request.limits
            },
            Limits {
                bytes: bytes - 1,
                ..request.limits
            },
            Limits {
                depth: 2,
                ..request.limits
            },
        ] {
            assert!(
                TreeRequest {
                    root: String::new(),
                    depth: 2,
                    limits
                }
                .build(&paths)
                .is_err()
            );
        }
        request.root = "a".into();
        assert_eq!(
            request.build(&paths).unwrap(),
            json!([{"x/":["n.md"]},"y.md"])
        );
        for name in ["ü😀", "quote\"slash\\", "\u{0001}\u{0008}\t\n\u{000c}\r"] {
            assert_eq!(
                string_bytes(name).unwrap(),
                serde_json::to_vec(name).unwrap().len()
            );
        }
    }

    #[test]
    fn deep_rejected_tree_never_materializes_recursive_values() {
        let path = format!("{}leaf.md", "a/".repeat(4096));
        let request = TreeRequest {
            root: String::new(),
            depth: 4097,
            limits: Limits {
                depth: 64,
                nodes: 20_000,
                bytes: 1024 * 1024,
            },
        };
        assert!(request.build(&[path]).is_err());
    }
}
