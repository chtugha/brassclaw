//! Serialized capture observations shared by admission readers and the native
//! producer. Parsing retains data only; it supplies no qualified Free source,
//! component approval, model permission or executable task capture.
use brassclaw_skills::recipe_model_input::PrefixFreePurpose;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PrefixFreeCaptureRecord {
    pub(super) format: String,
    pub(super) namespace: String,
    pub(super) recipe_uuid: Uuid,
    pub(super) recipe_version: u64,
    pub(super) recipe_checksum: String,
    pub(super) variant: String,
    pub(super) selection_checksum: String,
    pub(super) purpose: PrefixFreePurpose,
}
impl PrefixFreeCaptureRecord {
    pub(crate) fn parse(value: &Value, namespace: &str) -> Option<Self> {
        let record: Self = serde_json::from_value(value.clone()).ok()?;
        fn digest(s: &str) -> bool {
            s.len() == 64
                && s.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        }
        (record.format == "recipe-prefix-free-capture/1"
            && record.namespace == namespace
            && !namespace.is_empty()
            && !record.recipe_uuid.is_nil()
            && record.recipe_version > 0
            && digest(&record.recipe_checksum)
            && digest(&record.selection_checksum)
            && !record.variant.is_empty()
            && !record.variant.chars().any(char::is_control))
        .then_some(record)
    }
}
