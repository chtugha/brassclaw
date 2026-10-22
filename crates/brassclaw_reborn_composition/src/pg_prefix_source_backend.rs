//! Storage adapter over V143's existing immutable source/observation tables.
//! An appended observation is unqualified. Only the existing admitted Recipe's
//! trusted receipt/effect checks may later establish diagnostic provenance.
use async_trait::async_trait;
use brassclaw_host_api::ResourceScope;
use brassclaw_host_runtime::{
    PrefixSourceBackend, PrefixSourceBackendError as Error, PrefixSourceIdentity,
};
use brassclaw_pg::PgPool;
use brassclaw_skills::value_contract::{ContractLimits, strict_json};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::Arc;

pub(crate) struct PgPrefixSourceBackend {
    pool: Arc<PgPool>,
}
impl PgPrefixSourceBackend {
    pub(crate) fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}
fn sha(bytes: &str) -> String {
    hex::encode(Sha256::digest(bytes.as_bytes()))
}
async fn pinned(
    tx: &tokio_postgres::Transaction<'_>,
    source: &PrefixSourceIdentity,
) -> Result<String, Error> {
    if !source.validate() {
        return Err(Error::Invalid);
    }
    let revision = source.revision as i64;
    let row=tx.query_opt("SELECT registration_bytes,checksum,source_kind,canonical_reference FROM reborn_prefix_sources WHERE profile_name=$1 AND source_id=$2 AND revision=$3",
        &[&source.profile,&source.source_id,&revision]).await.map_err(|_|Error::Unavailable)?.ok_or(Error::NotFound)?;
    let bytes: String = row.get(0);
    let checksum: String = row.get(1);
    if checksum != source.registration_sha256 {
        return Err(Error::Conflict);
    }
    if sha(&bytes) != checksum {
        return Err(Error::Unavailable);
    }
    let registration = strict_json(
        &bytes,
        ContractLimits {
            max_bytes: 16384,
            max_nodes: 1024,
            max_depth: 16,
        },
    )
    .map_err(|_| Error::Unavailable)?;
    if registration["format"] != "brassclaw-prefix-source-registration/1"
        || registration["profile"] != source.profile
        || registration["kind"] != row.get::<_, String>(2)
        || registration["reference"] != row.get::<_, String>(3)
    {
        return Err(Error::Unavailable);
    }
    Ok(bytes)
}
#[async_trait]
impl PrefixSourceBackend for PgPrefixSourceBackend {
    async fn read(
        &self,
        source: &PrefixSourceIdentity,
        _scope: &ResourceScope,
    ) -> Result<Value, Error> {
        let mut client = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        let tx = client
            .build_transaction()
            .read_only(true)
            .start()
            .await
            .map_err(|_| Error::Unavailable)?;
        let bytes = pinned(&tx, source).await?;
        tx.commit().await.map_err(|_| Error::Unavailable)?;
        Ok(json!({"source":source,"registration_bytes":bytes}))
    }
    async fn retain_observation(
        &self,
        source: &PrefixSourceIdentity,
        diagnostic_bytes: &str,
        scope: &ResourceScope,
    ) -> Result<Value, Error> {
        // Exact original bytes are retained as data, without treating any claimed
        // success, workflow ID, timestamp or approval field as trusted evidence.
        strict_json(
            diagnostic_bytes,
            ContractLimits {
                max_bytes: 65536,
                max_nodes: 16384,
                max_depth: 32,
            },
        )
        .map_err(|_| Error::Invalid)?;
        let id = scope.invocation_id.as_uuid();
        if id.is_nil() {
            return Err(Error::Invalid);
        }
        let bytes = crate::prefix_generations::canonical(
            &json!({"format":"brassclaw-prefix-source-observation/1",
            "source":source,"native_invocation":id,"qualification":"unqualified",
            "diagnostic_bytes":diagnostic_bytes}),
        )
        .map_err(|_| Error::Invalid)?;
        if bytes.len() > 65536 {
            return Err(Error::Invalid);
        }
        let checksum = sha(&bytes);
        let mut client = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        let tx = client.transaction().await.map_err(|_| Error::Unavailable)?;
        pinned(&tx, source).await?;
        let revision = source.revision as i64;
        tx.execute("INSERT INTO reborn_prefix_source_observations(observation_id,profile_name,source_id,source_revision,observation_bytes,checksum) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT DO NOTHING",
            &[&id,&source.profile,&source.source_id,&revision,&bytes,&checksum]).await.map_err(|_|Error::Unavailable)?;
        let row=tx.query_one("SELECT profile_name,source_id,source_revision,observation_bytes,checksum FROM reborn_prefix_source_observations WHERE observation_id=$1",&[&id]).await.map_err(|_|Error::Unavailable)?;
        if row.get::<_, String>(0) != source.profile
            || row.get::<_, uuid::Uuid>(1) != source.source_id
            || row.get::<_, i64>(2) != revision
            || row.get::<_, String>(3) != bytes
            || row.get::<_, String>(4) != checksum
        {
            return Err(Error::Conflict);
        }
        tx.commit().await.map_err(|_| Error::Unavailable)?;
        Ok(json!({"observation_id":id,"checksum":checksum,"qualification":"unqualified"}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brassclaw_host_api::{InvocationId, TenantId, UserId};
    use brassclaw_product_workflow::{RegisterPrefixSourceRequest, WebUiAuthenticatedCaller};

    #[tokio::test]
    async fn source_storage_pins_registrations_and_keeps_observations_unqualified_and_immutable() {
        let rig = crate::runtime::test_pg::pg_rig().await;
        crate::prefix_profile_seeds::retain(&rig.pool)
            .await
            .unwrap();
        let sources = crate::prefix_sources::PgPrefixSources::new(rig.pool.clone());
        let caller = WebUiAuthenticatedCaller::new(
            TenantId::new("default").unwrap(),
            UserId::new("source-operator").unwrap(),
            None,
            None,
        );
        let registration = sources
            .register(
                "sempai",
                RegisterPrefixSourceRequest {
                    reference: "https://example.invalid/docs?v=3#api".into(),
                    title: "Versioned original".into(),
                    applicability: "Source diagnostics storage acceptance".into(),
                    required: true,
                },
                &caller,
            )
            .await
            .unwrap();
        let source = PrefixSourceIdentity {
            profile: "sempai".into(),
            source_id: registration.id.parse().unwrap(),
            revision: registration.revision,
            registration_sha256: registration.checksum,
        };
        let backend = PgPrefixSourceBackend::new(rig.pool.clone());
        let scope =
            ResourceScope::local_default(caller.user_id.clone(), InvocationId::new()).unwrap();
        let read = backend.read(&source, &scope).await.unwrap();
        assert_eq!(read["source"], serde_json::to_value(&source).unwrap());
        assert_eq!(
            sha(read["registration_bytes"].as_str().unwrap()),
            source.registration_sha256
        );
        let mut wrong = source.clone();
        wrong.registration_sha256 = "0".repeat(64);
        assert!(matches!(
            backend.read(&wrong, &scope).await,
            Err(Error::Conflict)
        ));
        wrong = source.clone();
        wrong.revision += 1;
        assert!(matches!(
            backend.read(&wrong, &scope).await,
            Err(Error::NotFound)
        ));
        // Even a caller's success/qualification claims remain inert diagnostic data.
        let diagnostic =
            r#"{"availability":"available","qualification":"trusted","model_approval":true}"#;
        let receipt = backend
            .retain_observation(&source, diagnostic, &scope)
            .await
            .unwrap();
        assert_eq!(receipt["qualification"], "unqualified");
        assert_eq!(
            backend
                .retain_observation(&source, diagnostic, &scope)
                .await
                .unwrap(),
            receipt
        );
        assert!(matches!(
            backend.retain_observation(&source, "{}", &scope).await,
            Err(Error::Conflict)
        ));
        for invalid in ["{", r#"{"x":1,"x":2}"#] {
            assert!(matches!(
                backend.retain_observation(&source, invalid, &scope).await,
                Err(Error::Invalid)
            ));
        }
        let too_large = json!({"text":"x".repeat(65536)}).to_string();
        assert!(matches!(
            backend
                .retain_observation(&source, &too_large, &scope)
                .await,
            Err(Error::Invalid)
        ));
        let inventory = sources.inventory("sempai", None).await.unwrap();
        assert!(!inventory.checks_available);
        assert_eq!(inventory.registered[0].availability, "not-checked");
        assert_eq!(
            inventory.registered[0].review_status,
            "pending-collection-review"
        );
        let client = rig.pool.get().await.unwrap();
        let row=client.query_one("SELECT observation_bytes,checksum FROM reborn_prefix_source_observations WHERE observation_id=$1",&[&scope.invocation_id.as_uuid()]).await.unwrap();
        let bytes: String = row.get(0);
        assert_eq!(sha(&bytes), row.get::<_, String>(1));
        assert_eq!(receipt["checksum"], row.get::<_, String>(1));
        let observed: Value = serde_json::from_str(&bytes).unwrap();
        assert_eq!(observed["diagnostic_bytes"], diagnostic);
        assert_eq!(observed["qualification"], "unqualified");
        assert!(
            client
                .execute(
                    "UPDATE reborn_prefix_source_observations SET observation_bytes='{}'",
                    &[]
                )
                .await
                .is_err()
        );
        assert!(
            client
                .execute("DELETE FROM reborn_prefix_source_observations", &[])
                .await
                .is_err()
        );
        let count: i64 = client
            .query_one(
                "SELECT count(*) FROM reborn_prefix_source_observations",
                &[],
            )
            .await
            .unwrap()
            .get(0);
        assert_eq!(count, 1);
    }
}
