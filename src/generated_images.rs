//! Shared helpers for image-generation sentinel payloads.

use std::borrow::Cow;

const MAX_EMBEDDED_JSON_STRING_LAYERS: usize = 3;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedImageSentinel {
    pub(crate) value: serde_json::Value,
}

impl GeneratedImageSentinel {
    pub(crate) fn from_value(value: &serde_json::Value) -> Option<Self> {
        let value = normalize_embedded_json(value)?;
        if value.get("type").and_then(|v| v.as_str()) != Some("image_generated") {
            return None;
        }
        Some(Self {
            value: value.into_owned(),
        })
    }

    pub(crate) fn data_url(&self) -> Option<&str> {
        self.value.get("data").and_then(|v| v.as_str())
    }

    pub(crate) fn media_type(&self) -> Option<&str> {
        self.value
            .get("media_type")
            .or_else(|| self.value.get("mime_type"))
            .and_then(|v| v.as_str())
    }

    pub(crate) fn path(&self) -> Option<&str> {
        self.value.get("path").and_then(|v| v.as_str())
    }

    pub(crate) fn summary_for_context(&self) -> String {
        let media_type = self.media_type().unwrap_or("image");
        format!("Generated image ({media_type})")
    }
}

fn normalize_embedded_json(value: &serde_json::Value) -> Option<Cow<'_, serde_json::Value>> {
    let serde_json::Value::String(s) = value else {
        return Some(Cow::Borrowed(value));
    };

    let mut current = serde_json::from_str::<serde_json::Value>(s).ok()?;
    // Generated-image sentinels may be serialized more than once as they flow
    // through tool output, DB persistence, and history reconstruction. Unwrap a
    // few layers to tolerate that pipeline, but stop after a small fixed number
    // of rounds so malformed input cannot trigger unbounded reparsing.
    for _ in 1..MAX_EMBEDDED_JSON_STRING_LAYERS {
        match current {
            serde_json::Value::String(ref s) => {
                current = serde_json::from_str::<serde_json::Value>(s).ok()?;
            }
            _ => return Some(Cow::Owned(current)),
        }
    }
    if matches!(current, serde_json::Value::String(_)) {
        tracing::debug!(
            max_layers = MAX_EMBEDDED_JSON_STRING_LAYERS,
            "Generated image sentinel remained stringified after max unwrapping rounds"
        );
    }
    Some(Cow::Owned(current))
}
