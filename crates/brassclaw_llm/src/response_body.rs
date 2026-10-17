//! HTTP body capacity is checked on decoded bytes, before accumulating a reply.
//! There is no partial answer, truncation or implicit retry after a capacity error.
use bytes::Bytes;

#[derive(Debug, thiserror::Error)]
pub enum ResponseBodyError {
    #[error("provider response exceeded configured byte capacity ({limit})")]
    Capacity { limit: u64 },
    #[error("provider response body could not be read")]
    Transport(#[source] reqwest::Error),
}
impl ResponseBodyError {
    pub(crate) fn into_llm(self, provider: &str) -> crate::LlmError {
        match self {
            Self::Capacity { limit } => crate::LlmError::ResponseTooLarge {
                provider: provider.into(),
                limit,
            },
            Self::Transport(_) => crate::LlmError::RequestFailed {
                provider: provider.into(),
                reason: "provider response body could not be read".into(),
            },
        }
    }
}

pub(crate) fn check_length(
    response: &reqwest::Response,
    limit: Option<u64>,
) -> Result<(), ResponseBodyError> {
    if let Some(limit) = limit
        && response
            .content_length()
            .is_some_and(|length| length > limit)
    {
        return Err(ResponseBodyError::Capacity { limit });
    }
    Ok(())
}

/// Read decoded response bytes without truncation. A finite capacity is checked
/// before accumulation; transport errors retain their type without formatting
/// upstream URLs or payloads. The caller owns deadline and policy validation.
pub async fn read(
    mut response: reqwest::Response,
    limit: Option<u64>,
) -> Result<Bytes, ResponseBodyError> {
    check_length(&response, limit)?;
    let Some(limit) = limit else {
        return response.bytes().await.map_err(ResponseBodyError::Transport);
    };
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(ResponseBodyError::Transport)?
    {
        let length = body
            .len()
            .checked_add(chunk.len())
            .filter(|length| (*length as u64) <= limit)
            .ok_or(ResponseBodyError::Capacity { limit })?;
        // Geometric growth avoids copying the entire accumulated body for
        // every small chunk. Never reserve from untrusted Content-Length or
        // above the selected ceiling. Allocation failure terminates the call.
        if length > body.capacity() {
            let ceiling =
                usize::try_from(limit).map_err(|_| ResponseBodyError::Capacity { limit })?;
            let desired = body.capacity().saturating_mul(2).max(length).min(ceiling);
            body.try_reserve_exact(desired - body.len())
                .map_err(|_| ResponseBodyError::Capacity { limit })?;
        }
        body.extend_from_slice(&chunk);
    }
    Ok(Bytes::from(body))
}

pub(crate) async fn text(
    response: reqwest::Response,
    limit: Option<u64>,
    provider: &str,
) -> Result<String, crate::LlmError> {
    let body = read(response, limit)
        .await
        .map_err(|error| error.into_llm(provider))?;
    // Matches reqwest's JSON-compatible replacement of malformed UTF-8. The
    // byte limit applies before this conversion; provider decoding is unchanged.
    Ok(String::from_utf8_lossy(&body).into_owned())
}

/// Classify an HTTP rejection without exposing upstream data. Only context
/// failures need a bounded private body; other statuses release it unread.
pub(crate) async fn rejection(
    response: reqwest::Response,
    limit: Option<u64>,
    provider: &str,
) -> crate::LlmError {
    let status = response.status().as_u16();
    let retry_after =
        crate::retry::parse_retry_after_hint(response.headers().get(reqwest::header::RETRY_AFTER));
    if matches!(status, 400 | 413) {
        let body = match text(response, limit, provider).await {
            Ok(body) => body,
            Err(error) => return error,
        };
        if let Some(error) = crate::error::context_length_error(status, &body) {
            return error;
        }
    }
    match status {
        401 | 403 => crate::LlmError::AuthFailed {
            provider: provider.into(),
        },
        429 => crate::LlmError::RateLimited {
            provider: provider.into(),
            retry_after,
        },
        500..=599 => crate::LlmError::BadGateway {
            provider: provider.into(),
            status,
            retry_after,
        },
        _ => crate::LlmError::RequestRejected {
            provider: provider.into(),
            status,
        },
    }
}
