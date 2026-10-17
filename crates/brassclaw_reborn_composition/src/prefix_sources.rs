//! Immutable source registration and manifest projection. Saving makes no fetch,
//! model call, approval, source-selection change or generation invalidation.
use crate::prefix_generations::PgPrefixGenerations;
use base64::{Engine, engine::general_purpose::STANDARD};
use brassclaw_pg::PgPool;
use brassclaw_product_workflow::{
    InterceptorConfigServiceError as Error, PrefixSourceInventory, PrefixSourceRegistration,
    PrefixUsedSource, RegisterPrefixSourceRequest,
};
use brassclaw_skills::value_contract::{ContractLimits, strict_json};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use uuid::Uuid;
fn bad() -> Error {
    Error::InvalidRequest {
        reason: "invalid or unsupported source reference".into(),
    }
}
fn sha(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
fn registration(
    id: Uuid,
    checksum: String,
    bytes: &str,
) -> Result<PrefixSourceRegistration, Error> {
    let v = strict_json(
        bytes,
        ContractLimits {
            max_depth: 16,
            max_nodes: 1024,
            max_bytes: 16384,
        },
    )
    .map_err(|_| Error::Unavailable)?;
    if sha(bytes.as_bytes()) != checksum {
        return Err(Error::Unavailable);
    }
    let field = |name: &str| {
        v[name]
            .as_str()
            .map(str::to_owned)
            .ok_or(Error::Unavailable)
    };
    Ok(PrefixSourceRegistration {
        id: id.to_string(),
        revision: 1,
        kind: field("kind")?,
        reference: field("reference")?,
        title: field("title")?,
        applicability: field("applicability")?,
        required: v["required"].as_bool().ok_or(Error::Unavailable)?,
        review_status: "pending-collection-review".into(),
        availability: "not-checked".into(),
        checksum,
        registration_origin: v["origin"]["kind"].as_str().map(str::to_owned),
        registered_by: v["origin"]["actor"].as_str().map(str::to_owned),
    })
}
pub(crate) struct PgPrefixSources {
    pool: Arc<PgPool>,
}
impl PgPrefixSources {
    pub(crate) fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
    pub(crate) async fn register(
        &self,
        profile: &str,
        request: RegisterPrefixSourceRequest,
        caller: &brassclaw_product_workflow::WebUiAuthenticatedCaller,
    ) -> Result<PrefixSourceRegistration, Error> {
        if request.title.trim().is_empty()
            || request.title.chars().count() > 200
            || request.applicability.trim().is_empty()
            || request.applicability.chars().count() > 2000
            || request.reference.len() > 8192
        {
            return Err(bad());
        }
        let (kind, reference, component) = if request.reference.starts_with("component:") {
            let body = request
                .reference
                .strip_prefix("component:")
                .ok_or_else(bad)?;
            let (name, checksum) = body.split_once('#').ok_or_else(bad)?;
            let (uuid, version) = name.split_once('@').ok_or_else(bad)?;
            let id = Uuid::parse_str(uuid)
                .ok()
                .filter(|id| !id.is_nil() && id.to_string() == uuid)
                .ok_or_else(bad)?;
            let number = version
                .parse::<i64>()
                .ok()
                .filter(|n| *n > 0 && n.to_string() == version)
                .ok_or_else(bad)?;
            if checksum.len() != 64
                || !checksum
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err(bad());
            }
            (
                "component",
                request.reference.clone(),
                Some((id, number, checksum.to_owned())),
            )
        } else {
            let url = url::Url::parse(&request.reference).map_err(|_| bad())?;
            if !matches!(url.scheme(), "http" | "https")
                || url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
            {
                return Err(bad());
            }
            if url.query_pairs().any(|(key, _)| {
                matches!(
                    key.to_ascii_lowercase().as_str(),
                    "token"
                        | "api_key"
                        | "apikey"
                        | "access_token"
                        | "password"
                        | "secret"
                        | "authorization"
                )
            }) {
                return Err(bad());
            }
            ("web", url.to_string(), None)
        };
        let bytes=json!({"format":"brassclaw-prefix-source-registration/1","profile":profile,"kind":kind,"reference":reference,"title":request.title,"applicability":request.applicability,"required":request.required,
            "origin":{"kind":"operator","actor":caller.user_id.as_str()}}).to_string();
        if bytes.len() > 16384 {
            return Err(bad());
        }
        let id = Uuid::new_v5(
            &Uuid::from_u128(0xb20a716a_23cf_414a_a673_95bbd95a3d45),
            format!("{profile}\0{reference}").as_bytes(),
        );
        let checksum = sha(bytes.as_bytes());
        let mut client = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        let tx = client.transaction().await.map_err(|_| Error::Unavailable)?;
        if tx
            .query_opt(
                "SELECT name FROM reborn_prefix_profiles WHERE name=$1 FOR SHARE",
                &[&profile],
            )
            .await
            .map_err(|_| Error::Unavailable)?
            .is_none()
        {
            return Err(Error::PrefixNotFound {
                name: profile.into(),
            });
        }
        if let Some((uuid, version, expected)) = component {
            let row=tx.query_opt("SELECT revision_bytes,checksum FROM reborn_component_revisions WHERE component_id=$1 AND version=$2",&[&uuid,&version]).await.map_err(|_|Error::Unavailable)?.ok_or_else(bad)?;
            if row.get::<_, String>(1) != expected
                || sha(row.get::<_, String>(0).as_bytes()) != expected
            {
                return Err(bad());
            }
        }
        tx.execute("INSERT INTO reborn_prefix_sources(profile_name,source_id,source_kind,canonical_reference,registration_bytes,checksum) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT DO NOTHING",&[&profile,&id,&kind,&reference,&bytes,&checksum]).await.map_err(|_|Error::Unavailable)?;
        let row=tx.query_one("SELECT source_id,registration_bytes,checksum FROM reborn_prefix_sources WHERE profile_name=$1 AND canonical_reference=$2",&[&profile,&reference]).await.map_err(|_|Error::Unavailable)?;
        if row.get::<_, Uuid>(0) != id
            || row.get::<_, String>(1) != bytes
            || row.get::<_, String>(2) != checksum
        {
            return Err(Error::Conflict);
        }
        tx.commit().await.map_err(|_| Error::Unavailable)?;
        registration(id, checksum, &bytes)
    }
    pub(crate) async fn inventory(
        &self,
        profile: &str,
        generation: Option<&str>,
    ) -> Result<PrefixSourceInventory, Error> {
        let client = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        if client
            .query_opt(
                "SELECT name FROM reborn_prefix_profiles WHERE name=$1",
                &[&profile],
            )
            .await
            .map_err(|_| Error::Unavailable)?
            .is_none()
        {
            return Err(Error::PrefixNotFound {
                name: profile.into(),
            });
        }
        let rows=client.query("SELECT source_id,checksum,registration_bytes FROM reborn_prefix_sources WHERE profile_name=$1 ORDER BY canonical_reference,source_id",&[&profile]).await.map_err(|_|Error::Unavailable)?;
        let registered = rows
            .iter()
            .map(|r| registration(r.get(0), r.get(1), &r.get::<_, String>(2)))
            .collect::<Result<Vec<_>, _>>()?;
        drop(client);
        let mut used = Vec::new();
        if let Some(generation) = generation {
            let detail = PgPrefixGenerations::new(self.pool.clone())
                .get(profile, generation)
                .await?;
            if detail.summary.kind == "compiler-package" {
                // The shared store already verifies the whole package and its
                // original/evidence identities. This merely projects its index.
                let package: Value = serde_json::from_str(&detail.generation_bytes)
                    .map_err(|_| Error::Unavailable)?;
                let sources = STANDARD
                    .decode(
                        package["artifacts"]["sources.json"]["data"]
                            .as_str()
                            .ok_or(Error::Unavailable)?,
                    )
                    .map_err(|_| Error::Unavailable)?;
                let source: Value =
                    serde_json::from_slice(&sources).map_err(|_| Error::Unavailable)?;
                for record in source["sources"].as_array().ok_or(Error::Unavailable)? {
                    let field = |name: &str| {
                        record[name]
                            .as_str()
                            .map(str::to_owned)
                            .ok_or(Error::Unavailable)
                    };
                    used.push(PrefixUsedSource {
                        reference: field("path")?,
                        checksum: field("sha256")?,
                        disposition: field("disposition")?,
                        reason: field("reason")?,
                        archived: true,
                    });
                }
            }
            // Legacy bundles contain no recoverable source inventory. Do not
            // invent sources by scanning text or mutable current component rows.
        }
        Ok(PrefixSourceInventory {
            profile: profile.into(),
            generation: generation.map(str::to_owned),
            used,
            registered,
            checks_available: false,
            check_blocked_reason: Some("prefix.sourceCheckPrerequisites".into()),
        })
    }
}
