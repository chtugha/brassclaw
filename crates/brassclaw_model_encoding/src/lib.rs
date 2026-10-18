//! Fixed offline model encoding. This library performs no provider calls,
//! source/design selection, publication, approval or cache qualification.
//! Production callers must use the retained bounded worker, not run untrusted
//! tokenizer/template workloads on the orchestration thread.
mod context;
mod python_json;
mod render;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokenizers::Tokenizer;

pub const PROTOCOL: &str = "brassclaw-model-encoding-worker/1";
pub const MAX_INPUT_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_RENDER_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_PROBES: usize = 64;

/// Public failure categories contain no source, request values or exception text.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case")]
#[error("{self:?}")]
pub enum TemplateFailure {
    Syntax,
    MissingTemplate,
    InvalidOperation,
    Arguments,
    UnknownFilter,
    UnknownTest,
    UnknownFunction,
    UnknownMethod,
    Undefined,
    Escape,
    Serialization,
    Unpack,
    Other,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum EncodingError {
    #[error("invalid encoding input")]
    InvalidInput,
    #[error("unsupported or invalid tokenizer")]
    Tokenizer,
    #[error("template evaluation failed ({kind})")]
    Template { kind: TemplateFailure },
    #[error("encoding resource capacity exceeded")]
    Capacity,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    pub tokenizer_json: String,
    pub tokenizer_config_json: String,
    pub chat_template: String,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Probe {
    /// No extra BOS/EOS insertion; complete chat templates already contain it.
    Text { text: String },
    /// Exact ordered render context, as serialized by the request codec.
    Chat { context_json: String },
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub format: String,
    pub assets: Assets,
    pub probes: Vec<Probe>,
}
#[derive(Serialize, Deserialize)]
pub struct ProbeEncoding {
    pub rendered_sha256: String,
    pub rendered_bytes: u64,
    pub token_count: u64,
    /// SHA-256 of consecutive little-endian u32 token IDs, without a header.
    pub token_ids_sha256: String,
}
#[derive(Serialize, Deserialize)]
pub struct EncodingReport {
    pub format: String,
    pub tokenizer_sha256: String,
    pub tokenizer_config_sha256: String,
    pub chat_template_sha256: String,
    pub truncation_disabled: bool,
    pub padding_disabled: bool,
    pub probes: Vec<ProbeEncoding>,
    pub common_prefix_tokens: u64,
    pub common_prefix_sha256: String,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Response {
    Ok { report: EncodingReport },
    Error { kind: EncodingError },
}
pub fn checksum(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
fn ids_checksum(ids: &[u32]) -> String {
    let mut digest = Sha256::new();
    for id in ids {
        digest.update(id.to_le_bytes());
    }
    hex::encode(digest.finalize())
}
pub fn analyze(request: Request) -> Result<EncodingReport, EncodingError> {
    if request.format != PROTOCOL || request.probes.is_empty() || request.probes.len() > MAX_PROBES
    {
        return Err(EncodingError::InvalidInput);
    }
    let Assets {
        tokenizer_json,
        tokenizer_config_json,
        chat_template,
    } = request.assets;
    let tokenizer_sha256 = checksum(tokenizer_json.as_bytes());
    let tokenizer_config_sha256 = checksum(tokenizer_config_json.as_bytes());
    let chat_template_sha256 = checksum(chat_template.as_bytes());
    let total = tokenizer_json
        .len()
        .checked_add(tokenizer_config_json.len())
        .and_then(|n| n.checked_add(chat_template.len()))
        .ok_or(EncodingError::Capacity)?;
    let total = request.probes.iter().try_fold(total, |total, probe| {
        total
            .checked_add(match probe {
                Probe::Text { text } => text.len(),
                Probe::Chat { context_json } => context_json.len(),
            })
            .ok_or(EncodingError::Capacity)
    })?;
    if total > MAX_INPUT_BYTES {
        return Err(EncodingError::Capacity);
    }
    let mut tokenizer =
        Tokenizer::from_bytes(tokenizer_json.as_bytes()).map_err(|_| EncodingError::Tokenizer)?;
    drop(tokenizer_json);
    // Serialized tokenizer defaults can silently truncate to 1024 tokens.
    // Measurements must observe the full input and never pad it.
    tokenizer
        .with_truncation(None)
        .map_err(|_| EncodingError::Tokenizer)?;
    tokenizer.with_padding(None);
    let config = context::parse(&tokenizer_config_json)?;
    let split_special = config
        .get_item(&minijinja::Value::from("split_special_tokens"))
        .map_err(|_| EncodingError::InvalidInput)?;
    if !split_special.is_undefined() && !split_special.is_none() {
        tokenizer.set_encode_special_tokens(
            bool::try_from(split_special).map_err(|_| EncodingError::InvalidInput)?,
        );
    }
    let renderer = render::Renderer::new(&chat_template, config)?;
    let mut results = Vec::with_capacity(request.probes.len());
    let mut common: Option<Vec<u32>> = None;
    for probe in request.probes {
        let text = match probe {
            Probe::Text { text } => text,
            Probe::Chat { context_json } => renderer.render(&context_json)?,
        };
        if text.len() > MAX_RENDER_BYTES {
            return Err(EncodingError::Capacity);
        }
        let encoding = tokenizer
            .encode(text.as_str(), false)
            .map_err(|_| EncodingError::Tokenizer)?;
        let ids = encoding.get_ids();
        match &mut common {
            None => common = Some(ids.to_vec()),
            Some(prefix) => {
                let length = prefix.iter().zip(ids).take_while(|(a, b)| a == b).count();
                prefix.truncate(length);
            }
        }
        results.push(ProbeEncoding {
            rendered_sha256: checksum(text.as_bytes()),
            rendered_bytes: text.len() as u64,
            token_count: ids.len() as u64,
            token_ids_sha256: ids_checksum(ids),
        });
    }
    let common = common.ok_or(EncodingError::InvalidInput)?;
    Ok(EncodingReport {
        format: "brassclaw-model-encoding/1".into(),
        tokenizer_sha256,
        tokenizer_config_sha256,
        chat_template_sha256,
        truncation_disabled: true,
        padding_disabled: true,
        probes: results,
        common_prefix_tokens: common.len() as u64,
        common_prefix_sha256: ids_checksum(&common),
    })
}

#[cfg(test)]
mod tests;

/// Worker-only deterministic single-thread encoding. Call before workloads start.
pub fn disable_parallelism() {
    tokenizers::utils::parallelism::set_parallelism(false);
}

/// Bounded decode for host callers. Contexts stay strings until the contained
/// worker; reject an oversized probe array before allocating its whole tail.
pub fn parse_probes(bytes: &str) -> Result<Vec<Probe>, EncodingError> {
    use serde::de::{self, SeqAccess, Visitor};
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(EncodingError::Capacity);
    }
    struct Probes;
    impl<'de> Visitor<'de> for Probes {
        type Value = Vec<Probe>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("one bounded probe array")
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
            let mut values = Vec::new();
            while let Some(probe) = access.next_element::<Probe>()? {
                if values.len() == MAX_PROBES {
                    return Err(de::Error::custom("probe capacity"));
                }
                values.push(probe);
            }
            if values.is_empty() {
                return Err(de::Error::custom("probes required"));
            }
            Ok(values)
        }
    }
    let mut reader = serde_json::Deserializer::from_str(bytes);
    let probes = serde::Deserializer::deserialize_seq(&mut reader, Probes)
        .map_err(|_| EncodingError::InvalidInput)?;
    reader.end().map_err(|_| EncodingError::InvalidInput)?;
    Ok(probes)
}
