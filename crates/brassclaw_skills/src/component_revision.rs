//! Exact retained revision envelope, distinct from Recipe/Skill authoring JSON.
//! A document contains all execution-relevant metadata; declared dependencies
//! use stable UUIDs. Exact selection supplies versions separately. Neither this
//! envelope nor a complete retained graph is evidence of activation/approval.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    association_contract::{AssociationError, ComponentRevisionRef, SkillAssociation},
    value_contract::{ContractError, ContractLimits, strict_json},
};

pub const REVISION_LIMITS: ContractLimits = ContractLimits {
    max_depth: 64,
    max_nodes: 262_144,
    max_bytes: 8_388_608,
};

#[derive(Debug, thiserror::Error)]
pub enum RevisionError {
    #[error("component revision: {0}")]
    Invalid(&'static str),
    #[error(transparent)]
    Data(#[from] ContractError),
    #[error(transparent)]
    Association(#[from] AssociationError),
}

/// Privately validated exact bytes. No Debug: authored documents may be private.
#[derive(Clone)]
pub struct ComponentRevisionDraft {
    bytes: String,
    uuid: Uuid,
    class_code: i32,
    checksum: [u8; 32],
    document: Value,
    dependencies: BTreeSet<Uuid>,
    association: Option<SkillAssociation>,
}

fn invalid(reason: &'static str) -> RevisionError {
    RevisionError::Invalid(reason)
}
fn component_uuid(value: &Value) -> Result<Uuid, RevisionError> {
    value
        .as_str()
        .and_then(|v| Uuid::parse_str(v).ok())
        .filter(|id| !id.is_nil())
        .ok_or_else(|| invalid("non-nil UUID required"))
}

impl ComponentRevisionDraft {
    /// Strict parsing retains exact bytes and rejects duplicate/unknown keys.
    /// The association belongs to the Skill revision and retains its own exact
    /// reviewed bytes; it is never inferred from prose or executable examples.
    pub fn from_json(bytes: &str) -> Result<Self, RevisionError> {
        let value = strict_json(bytes, REVISION_LIMITS)?;
        let record = value
            .as_object()
            .ok_or_else(|| invalid("object required"))?;
        let fields = [
            "format",
            "uuid",
            "class_code",
            "document",
            "dependencies",
            "association",
        ];
        if record.len() != fields.len()
            || fields.iter().any(|name| !record.contains_key(*name))
            || record["format"] != "component-revision/1"
        {
            return Err(invalid("unsupported revision envelope"));
        }
        let uuid = component_uuid(&record["uuid"])?;
        let class_code = record["class_code"]
            .as_i64()
            .and_then(|v| i32::try_from(v).ok())
            .filter(|v| matches!(v, 0..=10 | 12..=23 | 50))
            .ok_or_else(|| invalid("unsupported integer component class"))?;
        let document = record["document"]
            .as_object()
            .ok_or_else(|| invalid("document object required"))?;
        // Retained existing-row documents must agree with their outer identity.
        if document
            .get("id")
            .is_some_and(|id| component_uuid(id).ok() != Some(uuid))
            || document
                .get("class_code")
                .is_some_and(|class| class.as_i64() != Some(i64::from(class_code)))
        {
            return Err(invalid("document identity differs from revision identity"));
        }
        let mut dependencies = BTreeSet::new();
        for id in record["dependencies"]
            .as_array()
            .ok_or_else(|| invalid("dependency array required"))?
        {
            let id = component_uuid(id)?;
            if id == uuid || !dependencies.insert(id) {
                return Err(invalid("self or duplicate dependency"));
            }
        }
        let association = if (1..=3).contains(&class_code) {
            let bytes = record["association"]
                .as_str()
                .ok_or_else(|| invalid("Skill association bytes required"))?;
            let association = SkillAssociation::from_json(bytes, REVISION_LIMITS)?;
            if association.skill_uuid() != uuid
                || [
                    association.python_code_uuid(),
                    association.tool_skill_uuid(),
                    association.tool_uuid(),
                ]
                .iter()
                .any(|id| !dependencies.contains(id))
            {
                return Err(invalid("association owner or dependencies differ"));
            }
            Some(association)
        } else {
            if !record["association"].is_null() {
                return Err(invalid("only usage Skills own associations"));
            }
            None
        };
        Ok(Self {
            bytes: bytes.to_owned(),
            uuid,
            class_code,
            checksum: Sha256::digest(bytes.as_bytes()).into(),
            document: record["document"].clone(),
            dependencies,
            association,
        })
    }
    pub fn uuid(&self) -> Uuid {
        self.uuid
    }
    pub fn class_code(&self) -> i32 {
        self.class_code
    }
    pub fn exact_bytes(&self) -> &str {
        &self.bytes
    }
    pub fn checksum(&self) -> [u8; 32] {
        self.checksum
    }
    pub fn document(&self) -> &Value {
        &self.document
    }
    pub fn dependencies(&self) -> &BTreeSet<Uuid> {
        &self.dependencies
    }
    pub fn association(&self) -> Option<&SkillAssociation> {
        self.association.as_ref()
    }

    pub fn at_version(self, version: u64) -> Result<RetainedComponentRevision, RevisionError> {
        if version == 0 || version > i64::MAX as u64 {
            return Err(invalid("unsupported positive revision number"));
        }
        Ok(RetainedComponentRevision {
            reference: ComponentRevisionRef {
                uuid: self.uuid,
                class_code: self.class_code,
                version,
                checksum: self.checksum,
            },
            draft: self,
        })
    }
}

#[derive(Clone)]
pub struct RetainedComponentRevision {
    reference: ComponentRevisionRef,
    draft: ComponentRevisionDraft,
}
impl RetainedComponentRevision {
    pub fn reference(&self) -> ComponentRevisionRef {
        self.reference
    }
    pub fn draft(&self) -> &ComponentRevisionDraft {
        &self.draft
    }
}

/// Complete rooted transitive graph, loaded from exact immutable references.
/// This deliberately has no approved/active flag or latest lookup. Catalogue
/// selection and trusted combination-evidence resolution are separate gates.
pub struct RetainedComponentSnapshot {
    roots: BTreeSet<Uuid>,
    revisions: BTreeMap<Uuid, RetainedComponentRevision>,
}
impl RetainedComponentSnapshot {
    pub fn new(
        roots: &[Uuid],
        revisions: Vec<RetainedComponentRevision>,
    ) -> Result<Self, RevisionError> {
        let mut root_set = BTreeSet::new();
        for root in roots {
            if root.is_nil() || !root_set.insert(*root) {
                return Err(invalid("invalid or duplicate snapshot root"));
            }
        }
        if root_set.is_empty() {
            return Err(invalid("snapshot roots required"));
        }
        let mut graph = BTreeMap::new();
        for revision in revisions {
            if graph.insert(revision.reference.uuid, revision).is_some() {
                return Err(invalid("one revision per UUID required"));
            }
        }
        let mut visiting = BTreeSet::new();
        let mut visited = BTreeSet::new();
        for root in &root_set {
            visit(*root, &graph, &mut visiting, &mut visited, 0)?;
        }
        if visited.len() != graph.len() {
            return Err(invalid("snapshot contains unrelated revisions"));
        }
        for revision in graph.values() {
            if let Some(association) = revision.draft.association() {
                for (id, class) in [
                    (association.python_code_uuid(), 22),
                    (association.tool_skill_uuid(), 13),
                    (association.tool_uuid(), 0),
                ] {
                    if graph.get(&id).map(|r| r.reference.class_code) != Some(class) {
                        return Err(invalid("association dependency has wrong class"));
                    }
                }
            }
        }
        Ok(Self {
            roots: root_set,
            revisions: graph,
        })
    }
    pub fn roots(&self) -> &BTreeSet<Uuid> {
        &self.roots
    }
    pub fn revisions(&self) -> &BTreeMap<Uuid, RetainedComponentRevision> {
        &self.revisions
    }
}
fn visit(
    id: Uuid,
    graph: &BTreeMap<Uuid, RetainedComponentRevision>,
    visiting: &mut BTreeSet<Uuid>,
    visited: &mut BTreeSet<Uuid>,
    depth: usize,
) -> Result<(), RevisionError> {
    if visited.contains(&id) {
        return Ok(());
    }
    if depth >= 64 {
        return Err(invalid("dependency graph exceeds supported depth"));
    }
    if !visiting.insert(id) {
        return Err(invalid("cyclic dependency graph"));
    }
    let revision = graph
        .get(&id)
        .ok_or_else(|| invalid("missing dependency revision"))?;
    for dependency in revision.draft.dependencies() {
        visit(*dependency, graph, visiting, visited, depth + 1)?;
    }
    visiting.remove(&id);
    visited.insert(id);
    Ok(())
}
