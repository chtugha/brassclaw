//! Weak retention of compatible first-feed compiler tables, never VM state.
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Weak},
};

use crate::{PythonArtifact, VmBounds, VmError, VmFailure};

#[derive(Default)]
pub(crate) struct ArtifactCache {
    entries: BTreeMap<([u8; 32], BTreeSet<String>), Weak<PythonArtifact>>,
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
        let key = (checksum, aliases.clone());
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

    pub(crate) fn prune(&mut self) {
        self.entries.retain(|_, value| value.strong_count() != 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::time::Duration;

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
