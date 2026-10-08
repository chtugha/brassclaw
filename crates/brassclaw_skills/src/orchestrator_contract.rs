//! Exact class-10 global-root definition, separate from usage Skills and Q1/Q2.
//! Parsing/source integrity is not protected-root trust, approval or activation.
//! No source fallback, labels, tool grants or task runtime values are supplied.
use std::collections::BTreeSet;

use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    association_contract::ComponentRevisionRef,
    component_revision::{ComponentRevisionDraft, RetainedComponentRevision, RevisionError},
};

const FORMAT: &str = "monty-global-orchestrator/1";

/// Authoring data for the neutral root's actual two-symbol compiler interface.
/// The store retains the complete envelope; review/boot resolves exact versions.
pub fn global_root_draft(
    uuid: Uuid,
    source: &str,
    ports: &BTreeSet<String>,
) -> Result<ComponentRevisionDraft, RevisionError> {
    validate_source_ports(source, ports)?;
    ComponentRevisionDraft::from_json(
        &json!({"format":"component-revision/1","uuid":uuid,"class_code":10,
            "document":{"format":FORMAT,"name":"orchestrator:global",
                "source":source,"source_checksum":checksum(source),
                "input_symbols":["host","worker_count"],"host_ports":ports},
            "dependencies":[],"association":null})
        .to_string(),
    )
}

/// Privately parsed immutable definition. This class-10 record is executable
/// root code, not Skill prose or an implicit Skill/PythonCode association.
pub struct GlobalRootDefinition {
    reference: ComponentRevisionRef,
    source: String,
    source_checksum: [u8; 32],
    ports: BTreeSet<String>,
}
impl GlobalRootDefinition {
    pub fn from_revision(revision: &RetainedComponentRevision) -> Result<Self, RevisionError> {
        let invalid = || RevisionError::Invalid("invalid global-root definition");
        let draft = revision.draft();
        if revision.reference().class_code != 10 || !draft.dependencies().is_empty() {
            return Err(invalid());
        }
        let document = draft.document().as_object().ok_or_else(invalid)?;
        let fields = [
            "format",
            "name",
            "source",
            "source_checksum",
            "input_symbols",
            "host_ports",
        ];
        if document.len() != fields.len()
            || fields.iter().any(|field| !document.contains_key(*field))
            || document["format"] != FORMAT
            || document["name"] != "orchestrator:global"
            || document["input_symbols"] != json!(["host", "worker_count"])
        {
            return Err(invalid());
        }
        let source = document["source"].as_str().ok_or_else(invalid)?;
        if document["source_checksum"].as_str() != Some(checksum(source).as_str()) {
            return Err(RevisionError::Invalid(
                "global-root source checksum differs",
            ));
        }
        let mut ports = BTreeSet::new();
        for port in document["host_ports"].as_array().ok_or_else(invalid)? {
            if !ports.insert(port.as_str().ok_or_else(invalid)?.to_owned()) {
                return Err(RevisionError::Invalid("duplicate global-root host port"));
            }
        }
        validate_source_ports(source, &ports)?;
        Ok(Self {
            reference: revision.reference(),
            source: source.to_owned(),
            source_checksum: Sha256::digest(source.as_bytes()).into(),
            ports,
        })
    }
    pub fn reference(&self) -> ComponentRevisionRef {
        self.reference
    }
    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn source_checksum(&self) -> [u8; 32] {
        self.source_checksum
    }
    pub fn ports(&self) -> &BTreeSet<String> {
        &self.ports
    }
}

fn checksum(source: &str) -> String {
    format!("{:x}", Sha256::digest(source.as_bytes()))
}
fn validate_source_ports(source: &str, ports: &BTreeSet<String>) -> Result<(), RevisionError> {
    if source.trim().is_empty()
        || ["await_next_task", "enter_task", "finish_task"]
            .iter()
            .any(|port| !ports.contains(*port))
        || ports.iter().any(|port| {
            !port
                .as_bytes()
                .first()
                .is_some_and(|first| first.is_ascii_alphabetic() || *first == b'_')
                || !port
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        })
    {
        return Err(RevisionError::Invalid(
            "global-root source/ports are incomplete",
        ));
    }
    Ok(())
}
