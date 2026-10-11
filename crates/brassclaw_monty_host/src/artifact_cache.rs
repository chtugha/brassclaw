//! Weak retention of compatible first-feed compiler tables, never VM state.
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Weak},
};

use crate::{PythonArtifact, VmBounds, VmError, VmFailure};
use sha2::{Digest, Sha256};

type ArtifactKey = ([u8; 32], BTreeSet<String>, usize, usize, usize);

#[derive(Default)]
pub(crate) struct ArtifactCache {
    entries: BTreeMap<ArtifactKey, Weak<PythonArtifact>>,
}
impl ArtifactCache {
    pub(crate) fn prepare(
        &mut self,
        source: Arc<str>,
        checksum: [u8; 32],
        aliases: BTreeSet<String>,
        bounds: VmBounds,
    ) -> Result<Arc<PythonArtifact>, VmError> {
        // A caller-supplied checksum cannot replace actual integrity validation.
        // Live source bounds and aliases remain effective on every cache hit.
        PythonArtifact::validate(&source, checksum, &aliases, bounds)?;
        self.prune();
        let bytes = source
            .len()
            .checked_add("\nresult\n".len())
            .ok_or_else(|| VmError::kind(VmFailure::SourceLimit))?;
        let key = (checksum, aliases.clone(), 1, bytes, source.len());
        if let Some(artifact) = self.entries.get(&key).and_then(Weak::upgrade) {
            if artifact.body.as_ref() != source.as_ref() {
                return Err(VmError::kind(VmFailure::Integrity));
            }
            return Ok(artifact);
        }
        let artifact = Arc::new(PythonArtifact::new(source, checksum, aliases, bounds)?);
        self.entries.insert(key, Arc::downgrade(&artifact));
        Ok(artifact)
    }

    /// Compile one complete dependency-first definition prefix. Keep the same
    /// per-definition feed/byte charges as separate loads, and check live bounds
    /// and every exact source even on a hit. No heap or task values are shared.
    pub(crate) fn prepare_preload(
        &mut self,
        definitions: &[crate::process_recipe::SelectedPython],
        bounds: VmBounds,
    ) -> Result<Arc<PythonArtifact>, VmError> {
        if !bounds.valid() || definitions.is_empty() {
            return Err(VmError::kind(VmFailure::InvalidBounds));
        }
        if definitions.len() > bounds.max_feeds {
            return Err(VmError::kind(VmFailure::SourceLimit));
        }
        let mut charged_bytes = 0usize;
        let mut max_part_bytes = 0usize;
        for definition in definitions {
            PythonArtifact::validate(
                &definition.source,
                definition.checksum,
                &definition.aliases,
                bounds,
            )?;
            if !definition.aliases.is_empty() {
                return Err(VmError::kind(VmFailure::InvalidBinding));
            }
            let part_bytes = definition
                .source
                .len()
                .checked_add("\nresult = None".len())
                .ok_or_else(|| VmError::kind(VmFailure::SourceLimit))?;
            max_part_bytes = max_part_bytes.max(part_bytes);
            charged_bytes = part_bytes
                .checked_add("\nresult\n".len())
                .and_then(|part| charged_bytes.checked_add(part))
                .filter(|total| *total <= bounds.max_compiled_source_bytes)
                .ok_or_else(|| VmError::kind(VmFailure::SourceLimit))?;
            if max_part_bytes > bounds.max_source_bytes {
                return Err(VmError::kind(VmFailure::SourceLimit));
            }
        }
        let mut body = String::with_capacity(charged_bytes);
        for definition in definitions {
            if !body.is_empty() {
                body.push('\n');
            }
            body.push_str(&definition.source);
            body.push_str("\nresult = None");
        }
        let checksum = Sha256::digest(body.as_bytes()).into();
        let key = (
            checksum,
            BTreeSet::new(),
            definitions.len(),
            charged_bytes,
            max_part_bytes,
        );
        self.prune();
        if let Some(artifact) = self.entries.get(&key).and_then(Weak::upgrade) {
            if artifact.body.as_ref() != body.as_str() {
                return Err(VmError::kind(VmFailure::Integrity));
            }
            return Ok(artifact);
        }
        let mut symbols = BTreeSet::new();
        for definition in definitions {
            let Some(library) =
                crate::source_structure::inspect(&definition.source, bounds)?.preload
            else {
                return Err(VmError::kind(VmFailure::InvalidBinding));
            };
            for name in library.functions.keys().chain(library.constants.iter()) {
                if matches!(name.as_str(), "host" | "inputs" | "result")
                    || !symbols.insert(name.clone())
                {
                    return Err(VmError::kind(VmFailure::InvalidBinding));
                }
            }
        }
        let artifact = Arc::new(PythonArtifact::compile(
            Arc::from(body),
            checksum,
            BTreeSet::new(),
            max_part_bytes,
            definitions.len(),
            charged_bytes,
        )?);
        self.entries.insert(key, Arc::downgrade(&artifact));
        Ok(artifact)
    }

    pub(crate) fn prune(&mut self) {
        self.entries.retain(|_, value| value.strong_count() != 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RecipeVm, VmBoundary};
    use brassclaw_resources::{
        LiveMontyTaskSettings, MontyTaskLimits, MontyTaskSettingsRevision, SharedMontyTaskBudget,
    };
    use sha2::{Digest, Sha256};
    use std::time::Duration;

    fn definition(source: &str) -> crate::process_recipe::SelectedPython {
        crate::process_recipe::SelectedPython {
            source: source.to_owned(),
            checksum: Sha256::digest(source.as_bytes()).into(),
            aliases: BTreeSet::new(),
        }
    }

    #[test]
    fn full_prefix_shares_code_but_isolates_tasks_and_preserves_live_charges() {
        let bounds = VmBounds {
            max_source_bytes: 80,
            max_compiled_source_bytes: 1024,
            max_feeds: 4,
            max_stdout_bytes: 1024,
            execution_slice: Duration::from_secs(1),
            max_value_depth: 16,
            max_value_nodes: 1024,
            max_value_bytes: 4096,
        };
        let definitions = [
            definition("def make_state():\n    return []"),
            definition("def render(state):\n    return len(state)"),
        ];
        let mut cache = ArtifactCache::default();
        let prefix = cache.prepare_preload(&definitions, bounds).unwrap();
        assert!(prefix.body.len() > bounds.max_source_bytes);
        let reused = cache.prepare_preload(&definitions, bounds).unwrap();
        assert!(Arc::ptr_eq(&prefix, &reused));
        let retained = Arc::downgrade(&prefix);
        let budget = || {
            SharedMontyTaskBudget::new(
                LiveMontyTaskSettings::new(MontyTaskSettingsRevision {
                    revision: 1,
                    limits: MontyTaskLimits {
                        max_compute_time: Duration::from_secs(600),
                        token_budgets_enabled: false,
                    },
                })
                .unwrap(),
            )
        };
        let complete = |boundary| match boundary {
            VmBoundary::Complete(value) => value,
            other => panic!("unexpected boundary: {other:?}"),
        };
        let mut left = RecipeVm::new(budget(), bounds).unwrap();
        let mut right = RecipeVm::new(budget(), bounds).unwrap();
        for (vm, artifact) in [(&mut left, prefix.clone()), (&mut right, reused.clone())] {
            assert!(complete(vm.start_step(artifact, serde_json::json!({})).unwrap()).is_null());
            assert_eq!(vm.feeds, 2);
            assert_eq!(vm.compiled_bytes, prefix.compiled_source_bytes);
            let source: Arc<str> = Arc::from("saved = make_state()\nresult = render(saved)");
            let artifact = cache
                .prepare(
                    source.clone(),
                    Sha256::digest(source.as_bytes()).into(),
                    BTreeSet::new(),
                    bounds,
                )
                .unwrap();
            assert_eq!(
                complete(vm.start_step(artifact, serde_json::json!({})).unwrap()),
                0
            );
        }
        let source: Arc<str> = Arc::from("saved.append(inputs['value'])\nresult = render(saved)");
        let artifact = cache
            .prepare(
                source.clone(),
                Sha256::digest(source.as_bytes()).into(),
                BTreeSet::new(),
                bounds,
            )
            .unwrap();
        assert_eq!(
            complete(
                left.start_step(artifact, serde_json::json!({"value": "private"}))
                    .unwrap()
            ),
            1
        );
        let source: Arc<str> = Arc::from("result = render(saved)");
        let artifact = cache
            .prepare(
                source.clone(),
                Sha256::digest(source.as_bytes()).into(),
                BTreeSet::new(),
                bounds,
            )
            .unwrap();
        assert_eq!(
            complete(
                right
                    .start_step(artifact.clone(), serde_json::json!({}))
                    .unwrap()
            ),
            0
        );
        assert_eq!(
            left.start_step(artifact.clone(), serde_json::json!({}))
                .unwrap_err()
                .failure,
            VmFailure::SourceLimit
        );
        left.update_bounds(VmBounds {
            max_feeds: 5,
            ..bounds
        });
        assert_eq!(
            complete(left.start_step(artifact, serde_json::json!({})).unwrap()),
            1
        );
        assert!(
            cache
                .prepare_preload(
                    &definitions,
                    VmBounds {
                        max_feeds: 1,
                        ..bounds
                    }
                )
                .is_err()
        );
        assert!(
            cache
                .prepare_preload(
                    &definitions,
                    VmBounds {
                        max_source_bytes: 20,
                        ..bounds
                    }
                )
                .is_err()
        );
        let successor = [
            definition("def make_state():\n    return []"),
            definition("def render(state):\n    return len(state) + 10"),
        ];
        let successor = cache.prepare_preload(&successor, bounds).unwrap();
        assert!(!Arc::ptr_eq(&prefix, &successor));
        drop(prefix);
        drop(reused);
        drop(successor);
        assert!(
            retained.upgrade().is_some(),
            "active contexts retain their pinned prefix"
        );
        drop(left);
        drop(right);
        cache.prune();
        assert!(retained.upgrade().is_none());
        assert!(cache.entries.is_empty());
    }

    #[test]
    fn full_prefix_rejects_initializers_conflicts_bindings_and_tampering() {
        let bounds = VmBounds {
            max_source_bytes: 1024,
            max_compiled_source_bytes: 4096,
            max_feeds: 10,
            max_stdout_bytes: 1024,
            execution_slice: Duration::from_secs(1),
            max_value_depth: 16,
            max_value_nodes: 1024,
            max_value_bytes: 4096,
        };
        let mut cache = ArtifactCache::default();
        for definitions in [
            vec![definition("state = host.forbidden()")],
            vec![
                definition("def usage():\n    return 1"),
                definition("def usage():\n    return 2"),
            ],
            vec![definition("result = None")],
        ] {
            assert_eq!(
                cache
                    .prepare_preload(&definitions, bounds)
                    .unwrap_err()
                    .failure,
                VmFailure::InvalidBinding
            );
        }
        let mut selected = definition("def usage():\n    return 1");
        selected.aliases.insert("forbidden".into());
        assert_eq!(
            cache
                .prepare_preload(&[selected], bounds)
                .unwrap_err()
                .failure,
            VmFailure::InvalidBinding
        );
        let mut selected = definition("def usage():\n    return 1");
        selected.checksum = [0; 32];
        assert_eq!(
            cache
                .prepare_preload(&[selected], bounds)
                .unwrap_err()
                .failure,
            VmFailure::Integrity
        );
        assert!(cache.entries.is_empty());
    }

    #[test]
    fn hits_recheck_integrity_live_bounds_and_aliases_and_release_ownership() {
        let bounds = VmBounds {
            max_source_bytes: 1024,
            max_compiled_source_bytes: 4096,
            max_feeds: 10,
            max_stdout_bytes: 1024,
            execution_slice: Duration::from_millis(5),
            max_value_depth: 16,
            max_value_nodes: 1024,
            max_value_bytes: 4096,
        };
        let source: Arc<str> = Arc::from("def usage(value):\n    return value\nresult = None");
        let checksum = Sha256::digest(source.as_bytes()).into();
        let aliases = BTreeSet::from(["first_tool".to_owned()]);
        let mut cache = ArtifactCache::default();
        let first = cache
            .prepare(source.clone(), checksum, aliases.clone(), bounds)
            .unwrap();
        let second = cache
            .prepare(source.clone(), checksum, aliases.clone(), bounds)
            .unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        let Err(error) = cache.prepare(
            source.clone(),
            checksum,
            aliases.clone(),
            VmBounds {
                max_source_bytes: source.len() - 1,
                ..bounds
            },
        ) else {
            panic!("live source reduction must affect cache hits");
        };
        assert_eq!(error.failure, VmFailure::SourceLimit);
        let Err(error) = cache.prepare(
            Arc::from("result = host.second_tool()"),
            checksum,
            aliases,
            bounds,
        ) else {
            panic!("caller checksum is not proof of source integrity");
        };
        assert_eq!(error.failure, VmFailure::Integrity);
        let different = cache
            .prepare(
                source,
                checksum,
                BTreeSet::from(["second_tool".into()]),
                bounds,
            )
            .unwrap();
        assert!(!Arc::ptr_eq(&first, &different));
        assert_eq!(
            different.bindings,
            BTreeSet::from(["second_tool".to_owned()])
        );
        let retained = Arc::downgrade(&first);
        drop(first);
        assert!(retained.upgrade().is_some());
        drop(second);
        drop(different);
        cache.prune();
        assert!(retained.upgrade().is_none());
        assert!(cache.entries.is_empty());
    }
}
