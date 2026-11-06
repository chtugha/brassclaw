//! Explicit immutable prefix-definition origins. Historical chat definitions
//! remain historical; new source-only compiler work uses real private identity.
use super::{Error, validate_scope};
use crate::installed_monty_catalogue::PreparationCapture;
use brassclaw_host_api::TenantId;
use brassclaw_threads::ThreadScope;
use brassclaw_turns::{TurnScope, private_work::PrivateExecutionContext};
use serde_json::Value;

#[derive(Clone)]
pub(crate) enum Origin {
    Chat {
        scope: TurnScope,
        thread_scope: ThreadScope,
    },
    Private {
        context: PrivateExecutionContext,
        preparation: Box<PreparationCapture>,
    },
}
impl PartialEq for Origin {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Chat {
                    scope: a,
                    thread_scope: b,
                },
                Self::Chat {
                    scope: c,
                    thread_scope: d,
                },
            ) => a == c && b == d,
            (
                Self::Private {
                    context: a,
                    preparation: b,
                },
                Self::Private {
                    context: c,
                    preparation: d,
                },
            ) => a == c && b.bytes() == d.bytes(),
            _ => false,
        }
    }
}
impl Origin {
    /// Request identity excludes newly offered internal preparation settings.
    /// Full PartialEq remains strict for original capture/integrity comparisons.
    pub(super) fn same_request_origin(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Chat {
                    scope: a,
                    thread_scope: b,
                },
                Self::Chat {
                    scope: c,
                    thread_scope: d,
                },
            ) => a == c && b == d,
            (Self::Private { context: a, .. }, Self::Private { context: b, .. }) => a == b,
            _ => false,
        }
    }
    pub(crate) fn tenant_id(&self) -> &TenantId {
        match self {
            Self::Chat { scope, .. } => &scope.tenant_id,
            Self::Private { context, .. } => &context.tenant_id,
        }
    }
    pub(crate) fn chat_scope(&self) -> Result<&TurnScope, Error> {
        match self {
            Self::Chat { scope, .. } => Ok(scope),
            Self::Private { .. } => Err(Error::Invalid),
        }
    }
    #[cfg(test)]
    pub(crate) fn thread_scope(&self) -> Result<&ThreadScope, Error> {
        match self {
            Self::Chat { thread_scope, .. } => Ok(thread_scope),
            Self::Private { .. } => Err(Error::Invalid),
        }
    }
    pub(crate) fn context(&self, actor: &str) -> Result<PrivateExecutionContext, Error> {
        match self {
            Self::Private { context, .. } => Ok(context.clone()),
            Self::Chat { scope, .. } => Ok(PrivateExecutionContext {
                tenant_id: scope.tenant_id.clone(),
                agent_id: scope.agent_id.clone(),
                project_id: scope.project_id.clone(),
                actor: Some(brassclaw_turns::TurnActor::new(
                    brassclaw_host_api::UserId::new(actor).map_err(|_| Error::Invalid)?,
                )),
            }),
        }
    }
    pub(crate) fn validate(&self, actor: &str) -> Result<(), Error> {
        match self {
            Self::Chat {
                scope,
                thread_scope,
            } => validate_scope(scope, thread_scope, actor),
            Self::Private { context, .. } => {
                if context.actor.as_ref().map(|actor| actor.user_id.as_str()) != Some(actor)
                    || context.agent_id.is_none()
                {
                    return Err(Error::Invalid);
                }
                Ok(())
            }
        }
    }
    pub(crate) fn format(&self) -> &'static str {
        match self {
            Self::Chat { .. } => "prefix-job-definition/1",
            Self::Private { .. } => "prefix-job-definition/2",
        }
    }
}

pub(super) fn decode(
    format: &str,
    scope: Option<Value>,
    thread_scope: Option<Value>,
    context: Option<Value>,
    preparation_bytes: Option<String>,
    preparation_checksum: Option<String>,
    actor: &str,
) -> Result<Origin, Error> {
    let origin = match format {
        "prefix-job-definition/1"
            if context.is_none()
                && preparation_bytes.is_none()
                && preparation_checksum.is_none() =>
        {
            Origin::Chat {
                scope: serde_json::from_value(scope.ok_or(Error::Invalid)?)
                    .map_err(|_| Error::Invalid)?,
                thread_scope: serde_json::from_value(thread_scope.ok_or(Error::Invalid)?)
                    .map_err(|_| Error::Invalid)?,
            }
        }
        "prefix-job-definition/2" if scope.is_none() && thread_scope.is_none() => Origin::Private {
            context: serde_json::from_value(context.ok_or(Error::Invalid)?)
                .map_err(|_| Error::Invalid)?,
            preparation: PreparationCapture::restore_original(
                &preparation_bytes.ok_or(Error::Invalid)?,
                &preparation_checksum.ok_or(Error::Invalid)?,
            )
            .map(Box::new)
            .map_err(|_| Error::Invalid)?,
        },
        _ => return Err(Error::Invalid),
    };
    origin.validate(actor)?;
    Ok(origin)
}
