//! Storage envelope for existing AuthFlow owners, never a public route DTO.
use brassclaw_auth::{AuthFlowCustody, AuthFlowRecord, OAuthCallbackAction};
use brassclaw_filesystem::{EntryBodyChecksum, RecordVersion};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct StoredAuthFlow {
    #[serde(flatten)]
    pub(crate) flow: AuthFlowRecord,
    #[serde(default, skip_serializing_if = "AuthFlowCustody::is_empty")]
    pub(crate) auth_custody: AuthFlowCustody,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) fs_callback_publication: Option<FilesystemCallbackPublication>,
}

/// Local CAS-only publication decision. This is never success or another issuer
/// attempt: the account's matching same-record receipt establishes publication.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FilesystemCallbackPublication {
    pub(crate) action: OAuthCallbackAction,
    pub(crate) final_flow: AuthFlowRecord,
    pub(crate) account_version: Option<RecordVersion>,
    /// New decisions pin the exact account entry as well as its reusable version.
    /// Historical decisions may read an already written matching receipt, but
    /// missing identity must never authorize a new version-only publication.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) account_body_checksum: Option<EntryBodyChecksum>,
    /// New decisions pin their requirement receipt before account publication.
    /// Historical decisions without this field never manufacture a new binding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) requirement_selection: Option<Box<brassclaw_auth::CredentialRequirementSelection>>,
}

impl StoredAuthFlow {
    /// Bind original usage metadata in the very first flow write. This method
    /// cannot attach or replace metadata on an already published original.
    pub(crate) fn with_requirement_setup(
        mut self,
        setup: Option<Box<brassclaw_auth::CredentialRequirementSetup>>,
    ) -> Result<Self, brassclaw_auth::AuthProductError> {
        if let Some(setup) = &setup {
            setup.verify_flow(&self.flow)?;
        }
        if self.auth_custody.oauth_setup.is_some()
            || self.auth_custody.requirement_setup.is_some()
            || self.auth_custody.requirement_selection.is_some()
        {
            return Err(brassclaw_auth::AuthProductError::BackendConflict);
        }
        self.auth_custody.requirement_setup = setup;
        Ok(self)
    }

    pub(crate) fn new(
        flow: AuthFlowRecord,
        snapshot: Option<brassclaw_auth::CredentialAccount>,
    ) -> Result<Self, brassclaw_auth::AuthProductError> {
        match (&flow.update_binding, &snapshot) {
            (None, None) => {}
            (Some(binding), Some(account)) => {
                if !brassclaw_auth::scope_matches(&flow.scope, &account.scope)
                    || account.provider != flow.provider
                    || account.id != binding.account_id
                {
                    return Err(brassclaw_auth::AuthProductError::CrossScopeDenied);
                }
                brassclaw_auth::domain::validate_bound_update_authority(account, binding)?;
            }
            _ => return Err(brassclaw_auth::AuthProductError::BackendUnavailable),
        }
        Ok(Self {
            flow,
            auth_custody: AuthFlowCustody {
                device_login: None,
                update_account_snapshot: snapshot,
                oauth_callback: None,
                oauth_setup: None,
                dcr_registration: None,
                dcr_legacy: None,
                requirement_setup: None,
                requirement_selection: None,
                requirement_source: None,
            },
            fs_callback_publication: None,
        })
    }
}
