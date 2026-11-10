//! Explicit final transport resolution from the existing captured LLM config.
//! A supported codec is not an accepted Prefix, model measurement or authority.
use crate::{LlmConfig, ProviderProtocol, final_request_guard::FinalRequestError};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FinalRequestProtocol {
    OpenAiChat,
    CodexResponses,
}
#[derive(Clone)]
pub struct ResolvedFinalModelTransport {
    pub protocol: FinalRequestProtocol,
    pub endpoint: reqwest::Url,
}
impl ResolvedFinalModelTransport {
    pub fn contract_id(&self) -> &'static str {
        match self.protocol {
            FinalRequestProtocol::OpenAiChat => "brassclaw-rig030-openai-chat/1",
            FinalRequestProtocol::CodexResponses => crate::codex_request::TRANSPORT,
        }
    }
    pub fn contract_checksum(&self) -> String {
        match self.protocol {
            FinalRequestProtocol::OpenAiChat => crate::final_chat_contract_checksum(),
            FinalRequestProtocol::CodexResponses => crate::codex_request::contract_checksum(),
        }
    }
    /// Exact native contract comparison, never approval by a transport label.
    pub fn matches_contract(&self, id: &str, checksum: &str) -> bool {
        id == self.contract_id() && checksum == self.contract_checksum()
    }
}

pub fn resolve(config: &LlmConfig) -> Result<ResolvedFinalModelTransport, FinalRequestError> {
    let (protocol, endpoint) = match config.resolved_protocol {
        Some(ProviderProtocol::OpenAiCodex) if config.provider.is_none() => {
            let codex = config
                .openai_codex
                .as_ref()
                .ok_or(FinalRequestError::UnsupportedTransport)?;
            (
                FinalRequestProtocol::CodexResponses,
                crate::codex_request::endpoint(&codex.api_base_url)?,
            )
        }
        Some(ProviderProtocol::OpenAiCompletions) if config.openai_codex.is_none() => {
            let registry = config
                .provider
                .as_ref()
                .ok_or(FinalRequestError::UnsupportedTransport)?;
            if registry.is_codex_chatgpt {
                return Err(FinalRequestError::UnsupportedTransport);
            }
            (
                FinalRequestProtocol::OpenAiChat,
                crate::final_chat_endpoint(registry)?,
            )
        }
        _ => return Err(FinalRequestError::UnsupportedTransport),
    };
    Ok(ResolvedFinalModelTransport { protocol, endpoint })
}
