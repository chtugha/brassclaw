//! Private transcript encoding retains provider replay data; public encoding omits it.

use serde::{Serialize, Serializer, ser::SerializeSeq};

use crate::{ProviderToolCallReferenceEnvelope, ThreadMessageRecord};

#[derive(Serialize)]
pub(crate) struct StoredThreadMessageRecord<'a> {
    #[serde(flatten)]
    record: &'a ThreadMessageRecord,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_result_provider_call: &'a Option<ProviderToolCallReferenceEnvelope>,
}

impl<'a> From<&'a ThreadMessageRecord> for StoredThreadMessageRecord<'a> {
    fn from(record: &'a ThreadMessageRecord) -> Self {
        Self {
            record,
            tool_result_provider_call: &record.tool_result_provider_call,
        }
    }
}

pub(crate) fn serialize_messages<S: Serializer>(
    records: &[ThreadMessageRecord],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let mut sequence = serializer.serialize_seq(Some(records.len()))?;
    for record in records {
        sequence.serialize_element(&StoredThreadMessageRecord::from(record))?;
    }
    sequence.end()
}
