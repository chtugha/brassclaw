//! PostgreSQL-backed [`RebornIdentityResolver`](crate::RebornIdentityResolver).
//!
//! The identity data lives in three tables created by V020:
//!   - `brassclaw_identities`            — 5-part key → user_id
//!   - `brassclaw_identity_users`        — user_id → email / display_name
//!   - `brassclaw_identity_email_index`  — (tenant_id, email_lower) → user_id
//!
//! Concurrency contract follows the `FilesystemRebornIdentityStore`: a
//! per-process lock on the identity key serializes same-key concurrent first
//! logins within the process; DB UNIQUE constraints and ON CONFLICT DO NOTHING
//! are the cross-process backstop.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};

use async_trait::async_trait;
use brassclaw_host_api::UserId;
use chrono::{SecondsFormat, Utc};
use deadpool_postgres::Pool;
use uuid::Uuid;

use crate::{
    ExternalIdentityKey, RebornIdentityError, RebornIdentityResolver, ResolveExternalIdentity,
    SurfaceKind,
};

/// PostgreSQL-backed canonical identity resolver.
pub struct PgRebornIdentityStore {
    pool: Pool,
    locks: Arc<Mutex<HashMap<String, Weak<tokio::sync::Mutex<()>>>>>,
}

impl PgRebornIdentityStore {
    pub fn new(pool: Pool) -> Self {
        Self {
            pool,
            locks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn connect(&self) -> Result<deadpool_postgres::Object, RebornIdentityError> {
        self.pool
            .get()
            .await
            .map_err(|error| RebornIdentityError::Backend(error.to_string()))
    }

    fn lock_for(&self, key: String) -> Arc<tokio::sync::Mutex<()>> {
        let mut locks = self
            .locks
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        locks.retain(|_, lock| lock.strong_count() > 0);
        if let Some(lock) = locks.get(&key).and_then(Weak::upgrade) {
            return lock;
        }
        let lock = Arc::new(tokio::sync::Mutex::new(()));
        locks.insert(key, Arc::downgrade(&lock));
        lock
    }

    /// Read the user_id already bound to the given identity key, or `None`.
    async fn identity_user(
        &self,
        key: &IdentityKeyRef<'_>,
    ) -> Result<Option<UserId>, RebornIdentityError> {
        let client = self.connect().await?;
        let row = client
            .query_opt(
                "SELECT user_id \
                 FROM brassclaw_identities \
                 WHERE tenant_id = $1 \
                   AND surface_kind = $2 \
                   AND provider_kind = $3 \
                   AND provider_instance_id = $4 \
                   AND external_subject_id = $5 \
                 LIMIT 1",
                &[
                    &key.tenant_id,
                    &key.surface_kind,
                    &key.provider_kind,
                    &key.provider_instance_id,
                    &key.external_subject_id,
                ],
            )
            .await
            .map_err(|error| RebornIdentityError::Backend(error.to_string()))?;
        match row {
            Some(row) => {
                let raw: String = row
                    .try_get("user_id")
                    .map_err(|error| RebornIdentityError::Backend(error.to_string()))?;
                Ok(Some(to_user_id(raw)?))
            }
            None => Ok(None),
        }
    }

    /// Insert an identity row idempotently; if the row already exists, return
    /// its user_id (the cross-process winner).
    async fn put_identity_reconciling(
        &self,
        key: &IdentityKeyRef<'_>,
        user_id: &UserId,
        identity: &ResolveExternalIdentity,
        now: &str,
    ) -> Result<UserId, RebornIdentityError> {
        let id = ulid::Ulid::new().to_string();
        let client = self.connect().await?;
        client
            .execute(
                "INSERT INTO brassclaw_identities \
                     (id, tenant_id, surface_kind, provider_kind, provider_instance_id, \
                      external_subject_id, user_id, email, email_verified, created_at) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) \
                     ON CONFLICT (tenant_id, surface_kind, provider_kind, \
                                  provider_instance_id, external_subject_id) DO NOTHING",
                &[
                    &id,
                    &key.tenant_id,
                    &key.surface_kind,
                    &key.provider_kind,
                    &key.provider_instance_id,
                    &key.external_subject_id,
                    &user_id.as_str(),
                    &identity.email.as_deref(),
                    &identity.email_verified,
                    &now,
                ],
            )
            .await
            .map_err(|error| RebornIdentityError::Backend(error.to_string()))?;

        // Re-read the winner's user_id (either ours or the racing inserter).
        let winner = self
            .identity_user(key)
            .await?
            .ok_or_else(|| {
                RebornIdentityError::Backend(
                    "identity record vanished during reconciliation".to_string(),
                )
            })?;
        Ok(winner)
    }
}

/// Borrowed 5-part identity key (avoids too-many-arguments clippy lint).
struct IdentityKeyRef<'a> {
    tenant_id: &'a str,
    surface_kind: &'a str,
    provider_kind: &'a str,
    provider_instance_id: &'a str,
    external_subject_id: &'a str,
}

#[async_trait]
impl RebornIdentityResolver for PgRebornIdentityStore {
    async fn resolve_or_create(
        &self,
        identity: ResolveExternalIdentity,
    ) -> Result<UserId, RebornIdentityError> {
        if identity.surface_kind == SurfaceKind::ChannelActor {
            return Err(RebornIdentityError::ChannelActorNotMintable);
        }

        let tenant = identity.tenant_id.as_str();
        let surface = identity.surface_kind.as_str();
        let provider = identity.provider_kind.as_str();
        let instance = identity
            .provider_instance_id
            .as_ref()
            .map(|v| v.as_str())
            .unwrap_or("");
        let subject = identity.external_subject_id.as_str();

        let key = IdentityKeyRef {
            tenant_id: tenant,
            surface_kind: surface,
            provider_kind: provider,
            provider_instance_id: instance,
            external_subject_id: subject,
        };

        // Fast path: returning identity.
        if let Some(user_id) = self.identity_user(&key).await? {
            return Ok(user_id);
        }

        let lower_email = verified_email_key(&identity);

        // Per-identity-key in-process lock to serialize concurrent first logins.
        let lock_key = format!("{tenant}:{surface}:{provider}:{instance}:{subject}");
        let lock = self.lock_for(lock_key);
        let _guard = lock.lock().await;

        // Re-check under lock.
        if let Some(user_id) = self.identity_user(&key).await? {
            return Ok(user_id);
        }

        let now = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);

        // Link by verified email to an existing user in the same tenant.
        if let Some(ref email) = lower_email {
            let client = self.connect().await?;
            let row = client
                .query_opt(
                    "SELECT user_id \
                     FROM brassclaw_identity_email_index \
                     WHERE tenant_id = $1 AND email_lower = $2 \
                     LIMIT 1",
                    &[&tenant, &email.as_str()],
                )
                .await
                .map_err(|error| RebornIdentityError::Backend(error.to_string()))?;
            if let Some(row) = row {
                let existing_user_id: String = row
                    .try_get("user_id")
                    .map_err(|error| RebornIdentityError::Backend(error.to_string()))?;
                let user_id = to_user_id(existing_user_id)?;
                return self
                    .put_identity_reconciling(&key, &user_id, &identity, &now)
                    .await;
            }
        }

        // New user — mint a candidate.
        let new_user_id = to_user_id(Uuid::new_v4().to_string())?;

        // Mint the user row first.
        {
            let client = self.connect().await?;
            client
                .execute(
                    "INSERT INTO brassclaw_identity_users \
                         (user_id, email, display_name, created_at) \
                         VALUES ($1, $2, $3, $4) \
                         ON CONFLICT (user_id) DO NOTHING",
                    &[
                        &new_user_id.as_str(),
                        &identity.email.as_deref(),
                        &identity.display_name.as_deref(),
                        &now.as_str(),
                    ],
                )
                .await
                .map_err(|error| RebornIdentityError::Backend(error.to_string()))?;
        }

        // Try to claim the verified-email index (first writer wins).
        let owner_user_id = match &lower_email {
            Some(email) => {
                let client = self.connect().await?;
                client
                    .execute(
                        "INSERT INTO brassclaw_identity_email_index \
                             (tenant_id, email_lower, user_id, created_at) \
                             VALUES ($1, $2, $3, $4) \
                             ON CONFLICT (tenant_id, email_lower) DO NOTHING",
                        &[
                            &tenant,
                            &email.as_str(),
                            &new_user_id.as_str(),
                            &now.as_str(),
                        ],
                    )
                    .await
                    .map_err(|error| RebornIdentityError::Backend(error.to_string()))?;

                // Read back the winner.
                let row = client
                    .query_opt(
                        "SELECT user_id \
                         FROM brassclaw_identity_email_index \
                         WHERE tenant_id = $1 AND email_lower = $2 \
                         LIMIT 1",
                        &[&tenant, &email.as_str()],
                    )
                    .await
                    .map_err(|error| RebornIdentityError::Backend(error.to_string()))?;
                let winner_str: String = row
                    .ok_or_else(|| {
                        RebornIdentityError::Backend(
                            "verified-email index vanished after insert".to_string(),
                        )
                    })?
                    .try_get("user_id")
                    .map_err(|error| RebornIdentityError::Backend(error.to_string()))?;
                to_user_id(winner_str)?
            }
            None => new_user_id.clone(),
        };

        self.put_identity_reconciling(&key, &owner_user_id, &identity, &now)
            .await
    }

    async fn lookup(
        &self,
        key: ExternalIdentityKey,
    ) -> Result<Option<UserId>, RebornIdentityError> {
        let instance_owned;
        let instance = match &key.provider_instance_id {
            Some(v) => v.as_str(),
            None => {
                instance_owned = String::new();
                instance_owned.as_str()
            }
        };
        let key_ref = IdentityKeyRef {
            tenant_id: key.tenant_id.as_str(),
            surface_kind: key.surface_kind.as_str(),
            provider_kind: key.provider_kind.as_str(),
            provider_instance_id: instance,
            external_subject_id: key.external_subject_id.as_str(),
        };
        self.identity_user(&key_ref).await
    }

    async fn bind(
        &self,
        key: ExternalIdentityKey,
        user_id: &UserId,
    ) -> Result<(), RebornIdentityError> {
        let tenant = key.tenant_id.as_str();
        let surface = key.surface_kind.as_str();
        let provider = key.provider_kind.as_str();
        let instance = key
            .provider_instance_id
            .as_ref()
            .map(|v| v.as_str())
            .unwrap_or("");
        let subject = key.external_subject_id.as_str();
        let now = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
        let id = ulid::Ulid::new().to_string();

        let client = self.connect().await?;
        client
            .execute(
                "INSERT INTO brassclaw_identities \
                     (id, tenant_id, surface_kind, provider_kind, provider_instance_id, \
                      external_subject_id, user_id, email, email_verified, created_at) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7, NULL, false, $8) \
                     ON CONFLICT (tenant_id, surface_kind, provider_kind, \
                                  provider_instance_id, external_subject_id) \
                     DO UPDATE SET user_id = EXCLUDED.user_id",
                &[
                    &id, &tenant, &surface, &provider, &instance,
                    &subject, &user_id.as_str(), &now.as_str(),
                ],
            )
            .await
            .map_err(|error| RebornIdentityError::Backend(error.to_string()))?;
        Ok(())
    }

    async fn adopt_migrated_identity(
        &self,
        identity: ResolveExternalIdentity,
        user_id: &UserId,
    ) -> Result<(), RebornIdentityError> {
        let tenant = identity.tenant_id.as_str();
        let surface = identity.surface_kind.as_str();
        let provider = identity.provider_kind.as_str();
        let instance = identity
            .provider_instance_id
            .as_ref()
            .map(|v| v.as_str())
            .unwrap_or("");
        let subject = identity.external_subject_id.as_str();
        let now = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
        let id = ulid::Ulid::new().to_string();

        // Seed identity row (DO NOTHING so a returning user's existing row wins).
        {
            let client = self.connect().await?;
            client
                .execute(
                    "INSERT INTO brassclaw_identities \
                         (id, tenant_id, surface_kind, provider_kind, provider_instance_id, \
                          external_subject_id, user_id, email, email_verified, created_at) \
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) \
                         ON CONFLICT (tenant_id, surface_kind, provider_kind, \
                                      provider_instance_id, external_subject_id) DO NOTHING",
                    &[
                        &id, &tenant, &surface, &provider, &instance, &subject,
                        &user_id.as_str(),
                        &identity.email.as_deref(),
                        &identity.email_verified,
                        &now.as_str(),
                    ],
                )
                .await
                .map_err(|error| RebornIdentityError::Backend(error.to_string()))?;
        }

        // Seed the verified-email index (DO NOTHING — first writer wins).
        if let Some(email) = verified_email_key(&identity) {
            let client = self.connect().await?;
            client
                .execute(
                    "INSERT INTO brassclaw_identity_email_index \
                         (tenant_id, email_lower, user_id, created_at) \
                         VALUES ($1, $2, $3, $4) \
                         ON CONFLICT (tenant_id, email_lower) DO NOTHING",
                    &[&tenant, &email.as_str(), &user_id.as_str(), &now.as_str()],
                )
                .await
                .map_err(|error| RebornIdentityError::Backend(error.to_string()))?;
        }

        Ok(())
    }
}

fn verified_email_key(identity: &ResolveExternalIdentity) -> Option<String> {
    if identity.surface_kind != SurfaceKind::Oauth || !identity.email_verified {
        return None;
    }
    identity
        .email
        .as_deref()
        .map(str::to_ascii_lowercase)
        .filter(|email| !email.is_empty())
}

fn to_user_id(raw: String) -> Result<UserId, RebornIdentityError> {
    UserId::new(raw).map_err(|error| RebornIdentityError::InvalidUserId(error.to_string()))
}
