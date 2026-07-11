//! OpenClaw conversation history import.
//!
//! Conversation tables were removed in the v2 Reborn migration. This module
//! is preserved as a no-op so the import feature can continue to compile and
//! report the correct stats without attempting to write to a non-existent
//! conversations table.

use uuid::Uuid;

use crate::db::Database;
use crate::import::{ImportError, ImportOptions};

use super::reader::OpenClawConversation;

/// Import a conversation. Currently a no-op because conversation tables were
/// removed in the v2 schema migration. Returns (nil_uuid, 0).
pub async fn import_conversation_atomic(
    _db: &std::sync::Arc<dyn Database>,
    _conv: OpenClawConversation,
    _opts: &ImportOptions,
) -> Result<(Uuid, usize), ImportError> {
    // Conversation history import is not supported in the v2 schema.
    Ok((Uuid::nil(), 0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::openclaw::reader::OpenClawMessage;

    #[test]
    fn test_conversation_import_structure() {
        // Verify that OpenClawConversation can be created with test data
        let conv = OpenClawConversation {
            id: "conv-123".to_string(),
            channel: "telegram".to_string(),
            created_at: None,
            messages: vec![
                OpenClawMessage {
                    role: "user".to_string(),
                    content: "Hello".to_string(),
                    created_at: None,
                },
                OpenClawMessage {
                    role: "assistant".to_string(),
                    content: "Hi there".to_string(),
                    created_at: None,
                },
            ],
        };

        assert_eq!(conv.id, "conv-123");
        assert_eq!(conv.messages.len(), 2);
        assert_eq!(conv.channel, "telegram");
    }
}
