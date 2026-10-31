//! Checked, immutable Pair/Profile/Generation references in the existing Prefix
//! acceptance owner. Storage labels, imported manifests and historical receipts
//! never become authority. No copied model limits or current-role lookup.
use crate::final_model_encoding::{FinalEncodingBindingVerifier, FinalEncodingSelection};
use async_trait::async_trait;
use brassclaw_host_api::ResourceScope;
use brassclaw_llm::final_request_guard::FinalRequestError;
use brassclaw_model_encoding::checksum;
use brassclaw_pg::PgPool;
use brassclaw_product_workflow::InterceptorConfigServiceError as Error;
use brassclaw_skills::value_contract::strict_json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

const FORMAT: &str = "brassclaw-prefix-acceptance/1";
pub(crate) const TRANSPORT: &str = "brassclaw-rig030-openai-chat/1";

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EncodingContract {
    pub(crate) specification: String,
    pub(crate) artifact_set: String,
    pub(crate) provider_id: String,
    pub(crate) model: String,
    pub(crate) pair_proposal: String,
    pub(crate) mapping_json: String,
    pub(crate) tokenizer_sha256: String,
    pub(crate) tokenizer_config_sha256: String,
    pub(crate) template_sha256: String,
    pub(crate) worker_sha256: String,
    pub(crate) transport: String,
    pub(crate) transport_checksum: String,
    pub(crate) endpoint: String,
    /// Explicit accepted request default, not another mutable model/input cap.
    pub(crate) request_output_tokens: u32,
}

/// Exact machine/native review output, embedded in the human's reviewed view.
/// It is constructed by the contained acceptance checks, never deserialized as
/// a successful write request. The persisted reader still checks its integrity.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReviewedTarget {
    pub(crate) namespace: String,
    pub(crate) contract: EncodingContract,
    pub(crate) profile: String,
    pub(crate) generation: String,
    pub(crate) profile_proposal: String,
    pub(crate) prefix_sha256: String,
    pub(crate) prefix_token_ids_sha256: String,
    pub(crate) checks: serde_json::Value,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Accepted {
    format: String,
    id: Uuid,
    scope: String,
    pair_parent: Option<Uuid>,
    profile_parent: Option<Uuid>,
    review: ReviewedTarget,
    review_checksum: String,
    actor: String,
    semantic_review: String,
}

pub(crate) struct PgPrefixAcceptances {
    pool: Arc<PgPool>,
    namespace: String,
}
/// Constructible only after the native checks below. In particular the HTTP
/// caller cannot deserialize a successful review or supply token measurements.
pub(crate) struct PreparedPrefixReview {
    target: ReviewedTarget,
    bytes: String,
    checksum: String,
}
impl PreparedPrefixReview {
    pub(crate) fn view(&self) -> serde_json::Value {
        serde_json::json!({"review":self.target,"review_checksum":self.checksum})
    }
}

impl PgPrefixAcceptances {
    pub(crate) fn new(pool: Arc<PgPool>, namespace: String) -> Self {
        Self { pool, namespace }
    }
    pub(crate) async fn endpoint(&self, id: Uuid) -> Result<reqwest::Url, Error> {
        reqwest::Url::parse(&self.record(id).await?.review.contract.endpoint)
            .map_err(|_| Error::Unavailable)
    }
    pub(crate) async fn request_output_tokens(&self, id: Uuid) -> Result<u32, Error> {
        let n = self.record(id).await?.review.contract.request_output_tokens;
        if n == 0 {
            return Err(Error::Unavailable);
        }
        Ok(n)
    }
    pub(crate) async fn reference(
        &self,
        id: Uuid,
        preparation_revision: u64,
    ) -> Result<Arc<str>, Error> {
        let selected = self.resolve(id, preparation_revision).await?;
        let package = super::prefix_generations::PgPrefixGenerations::new(self.pool.clone())
            .get(&selected.profile, &selected.generation)
            .await?;
        Ok(super::prefix_generation_binding::read(
            &selected.profile,
            &selected.generation,
            &package.generation_bytes,
        )?
        .reference)
    }
    pub(crate) async fn bindings(
        &self,
    ) -> Result<std::collections::BTreeMap<(String, String, String), Uuid>, Error> {
        let client = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        let rows = client.query("SELECT provider_id,model,profile_name,generation_qualification FROM reborn_prefix_target_bindings WHERE provider_namespace=$1 ORDER BY provider_id,model,profile_name", &[&self.namespace])
            .await.map_err(|_| Error::Unavailable)?;
        Ok(rows
            .into_iter()
            .map(|r| ((r.get(0), r.get(1), r.get(2)), r.get(3)))
            .collect())
    }
    /// Publication is separate from review. No regeneration, model call,
    /// fallback selection or mutation of an accepted artifact occurs here.
    pub(crate) async fn activate(
        &self,
        id: Uuid,
        expected_revision: u64,
        preparation_revision: u64,
    ) -> Result<u64, Error> {
        let selected = self.resolve(id, preparation_revision).await?;
        let expected = i64::try_from(expected_revision).map_err(|_| Error::Conflict)?;
        let mut client = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        let tx = client.transaction().await.map_err(|_| Error::Unavailable)?;
        let row = if expected == 0 {
            tx.query_opt("INSERT INTO reborn_prefix_target_bindings(provider_namespace,provider_id,model,profile_name,revision,generation_qualification) VALUES($1,$2,$3,$4,1,$5) ON CONFLICT DO NOTHING RETURNING revision",
                &[&self.namespace,&selected.provider_id,&selected.model,&selected.profile,&id]).await
        } else {
            tx.query_opt("UPDATE reborn_prefix_target_bindings SET revision=revision+1,generation_qualification=$5 WHERE provider_namespace=$1 AND provider_id=$2 AND model=$3 AND profile_name=$4 AND revision=$6 RETURNING revision",
                &[&self.namespace,&selected.provider_id,&selected.model,&selected.profile,&id,&expected]).await
        }.map_err(|_| Error::Conflict)?;
        let revision = match row {
            Some(row) => row.get::<_, i64>(0),
            None => {
                let row = tx.query_opt("SELECT revision,generation_qualification FROM reborn_prefix_target_bindings WHERE provider_namespace=$1 AND provider_id=$2 AND model=$3 AND profile_name=$4",
                    &[&self.namespace,&selected.provider_id,&selected.model,&selected.profile]).await.map_err(|_| Error::Unavailable)?.ok_or(Error::Conflict)?;
                let revision: i64 = row.get(0);
                if row.get::<_, Uuid>(1) != id || Some(revision) != expected.checked_add(1) {
                    return Err(Error::Conflict);
                }
                revision
            }
        };
        tx.commit().await.map_err(|_| Error::Unavailable)?;
        u64::try_from(revision).map_err(|_| Error::Unavailable)
    }
    pub(crate) async fn approved_retry(
        &self,
        decision: &brassclaw_product_workflow::PrefixAcceptanceDecision,
        actor: &str,
    ) -> Result<bool, Error> {
        let client = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        let exists = client.query_one("SELECT EXISTS(SELECT 1 FROM reborn_prefix_design_acceptances WHERE acceptance_id=$1)",
            &[&decision.acceptance_id]).await.map_err(|_| Error::Unavailable)?.get::<_,bool>(0);
        drop(client);
        if !exists {
            return Ok(false);
        }
        let old = self.record(decision.acceptance_id).await?;
        if old.scope != "generation"
            || old.review.profile != decision.selection.profile
            || old.review.generation != decision.selection.generation
            || old.review_checksum != decision.review_checksum
            || old.actor != actor
            || old.semantic_review != decision.semantic_review
        {
            return Err(Error::Conflict);
        }
        Ok(true)
    }
    /// Called only from authenticated operator composition after showing the
    /// exact native review. This writes acceptance, never a serving pointer.
    /// Deterministic parent identities share ONE pair design across profiles.
    pub(crate) async fn approve(
        &self,
        prepared: PreparedPrefixReview,
        id: Uuid,
        reviewed_checksum: &str,
        actor: &str,
        semantic_review: &str,
    ) -> Result<Uuid, Error> {
        if id.is_nil()
            || reviewed_checksum != prepared.checksum
            || prepared.target.namespace != self.namespace
            || checksum(prepared.bytes.as_bytes()) != prepared.checksum
            || actor.trim().is_empty()
            || actor.len() > 2048
            || semantic_review.trim().is_empty()
            || semantic_review.len() > 16_384
        {
            return Err(Error::Conflict);
        }
        let pair_id = Uuid::new_v5(
            &Uuid::NAMESPACE_OID,
            format!(
                "brassclaw:accepted-prefix-pair:{}",
                prepared.target.contract.pair_proposal
            )
            .as_bytes(),
        );
        let profile_id = Uuid::new_v5(&pair_id, prepared.target.profile_proposal.as_bytes());
        let mut client = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .start()
            .await
            .map_err(|_| Error::Unavailable)?;
        for (accepted_id, scope, pair_parent, profile_parent, proposal) in [
            (
                pair_id,
                "pair",
                None,
                None,
                &prepared.target.contract.pair_proposal,
            ),
            (
                profile_id,
                "profile",
                Some(pair_id),
                None,
                &prepared.target.profile_proposal,
            ),
            (
                id,
                "generation",
                Some(pair_id),
                Some(profile_id),
                &prepared.target.profile_proposal,
            ),
        ] {
            let evidence = Accepted {
                format: FORMAT.into(),
                id: accepted_id,
                scope: scope.into(),
                pair_parent,
                profile_parent,
                review: prepared.target.clone(),
                review_checksum: prepared.checksum.clone(),
                actor: actor.into(),
                semantic_review: semantic_review.into(),
            };
            let bytes = serde_json::to_string(&evidence).map_err(|_| Error::Unavailable)?;
            // A larger review cannot partially insert the parents and then fail.
            if bytes.len() > super::prefix_model_designs::PROPOSAL_LIMITS.max_bytes {
                return Err(Error::Conflict);
            }
            tx.execute("INSERT INTO reborn_prefix_design_acceptances(acceptance_id,proposal_id,candidate_profile,candidate_generation,evidence_bytes,checksum,acceptance_scope,pair_acceptance_ref,profile_acceptance_ref) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT(acceptance_id) DO NOTHING",
                &[&accepted_id,proposal,&prepared.target.profile,&prepared.target.generation,&bytes,&checksum(bytes.as_bytes()),&scope,&pair_parent,&profile_parent])
                .await.map_err(|_| Error::Conflict)?;
            let old = tx.query_one("SELECT evidence_bytes,checksum FROM reborn_prefix_design_acceptances WHERE acceptance_id=$1", &[&accepted_id])
                .await.map_err(|_| Error::Unavailable)?;
            let original: String = old.get(0);
            if checksum(original.as_bytes()) != old.get::<_, String>(1) {
                return Err(Error::Unavailable);
            }
            let old: Accepted = serde_json::from_value(
                strict_json(&original, super::prefix_model_designs::PROPOSAL_LIMITS)
                    .map_err(|_| Error::Unavailable)?,
            )
            .map_err(|_| Error::Unavailable)?;
            // Reusing an immutable accepted parent does not approve a new
            // design, reapprove unchanged dependencies or rewrite old evidence.
            if old.format != FORMAT
                || old.id != accepted_id
                || old.scope != scope
                || old.review.namespace != self.namespace
                || old.review.contract != prepared.target.contract
                || old.pair_parent != pair_parent
                || old.profile_parent != profile_parent
                || (scope != "pair"
                    && (old.review.profile != prepared.target.profile
                        || old.review.profile_proposal != prepared.target.profile_proposal))
                || (scope == "generation" && original != bytes)
            {
                return Err(Error::Conflict);
            }
        }
        tx.commit().await.map_err(|_| Error::Unavailable)?;
        Ok(id)
    }

    /// Mechanical Q1/behavior checks over immutable data, using one operation
    /// on the existing contained encoder. No model, Tool workflow or server
    /// configuration change. Human semantic/default-contract review is separate.
    pub(crate) async fn prepare(
        &self,
        profile: &str,
        generation: &str,
        scope: &ResourceScope,
        backend: &dyn brassclaw_host_runtime::ModelEncodingBackend,
        governor: Arc<dyn brassclaw_resources::ResourceGovernor>,
    ) -> Result<PreparedPrefixReview, Error> {
        use brassclaw_model_encoding::{EncodingReport, Probe};
        use serde_json::json;
        let bad = || Error::Conflict;
        if scope.tenant_id.as_str() != self.namespace {
            return Err(bad());
        }
        let package = super::prefix_generations::PgPrefixGenerations::new(self.pool.clone())
            .get(profile, generation)
            .await?;
        let artifact =
            super::prefix_generation_binding::read(profile, generation, &package.generation_bytes)?;
        let b = &artifact.binding;
        let designs = super::prefix_model_designs::PgPrefixModelDesigns::new(
            self.pool.clone(),
            self.namespace.clone(),
        );
        let specification = designs.get_specification(&b.specification).await?;
        let pair = designs
            .get_pair_proposal(&b.specification, &b.pair_proposal)
            .await?;
        let document =
            super::prefix_pair_construction::parse(&b.specification, &pair.proposal_bytes)?;
        let mapping = document.wire_mapping_json().ok_or_else(bad)?;
        let request_output_tokens = document
            .request_output_tokens()
            .filter(|n| *n > 0)
            .ok_or_else(bad)?;
        artifact.require_references(
            &b.specification,
            &b.artifact_set,
            &b.pair_proposal,
            &b.profile_proposal,
            mapping,
        )?;
        let content = designs.get_proposal(profile, &b.profile_proposal).await?;
        if content.specification != b.specification
            || artifact.reference.is_empty()
            || document.fixed_prefix(&artifact.reference) != artifact.prefix.as_ref()
            || document.case_count() == 0
            || document.review_cases().count() != document.case_count()
        {
            return Err(bad());
        }
        let assets = super::prefix_model_artifacts::PgPrefixModelArtifacts::new(
            self.pool.clone(),
            self.namespace.clone(),
        )
        .get(&b.specification, &b.artifact_set)
        .await?;
        super::prefix_pair_construction::verify_sources(&document, &assets)?;
        let client = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        let original = super::pg_provider_repo::PgProviderRepo::new(
            self.pool.as_ref().clone(),
            &self.namespace,
        )
        .read_retained_revision(
            &**client,
            &specification.provider_id,
            specification.provider_revision,
        )
        .await
        .map_err(|_| Error::Unavailable)?
        .ok_or_else(bad)?;
        drop(client);
        let mut client_config = brassclaw_llm::RegistryProviderConfig::generic(
            original.definition.protocol,
            &specification.provider_id,
            None,
            original
                .definition
                .default_base_url
                .as_deref()
                .ok_or_else(bad)?,
            &specification.model,
        );
        client_config.unsupported_params = original.definition.unsupported_params.clone();
        client_config.cache_retention = original
            .definition
            .cache_retention
            .as_deref()
            .map(str::parse::<brassclaw_llm::CacheRetention>)
            .transpose()
            .map_err(|_| bad())?
            .unwrap_or_default();
        let endpoint = brassclaw_llm::final_chat_endpoint(&client_config).map_err(|_| bad())?;
        let sdk_bodies = brassclaw_llm::request_qualification::prefix_bodies(
            &client_config,
            original.definition.request_policy.timeout_secs,
            &artifact.reference,
            request_output_tokens,
            document.supports_mode("tools"),
            brassclaw_model_encoding::MAX_INPUT_BYTES,
        )
        .await
        .map_err(|_| bad())?;

        let mut probes = Vec::new();
        let mut expected = Vec::new();
        for (id, wire, prefix, rendered) in document.review_cases() {
            // Q1 additionally checks the explicit SDK mapping, not just the
            // author's independently supplied render_context_json.
            brassclaw_model_encoding::wire::project(wire, mapping, &specification.model)
                .map_err(|_| bad())?;
            probes.push(Probe::WireChat {
                request_json: wire.into(),
                mapping_json: mapping.into(),
                expected_model: specification.model.clone(),
                leading_prefix: prefix.into(),
            });
            probes.push(Probe::Text {
                text: rendered.into(),
            });
            expected.push((id.to_owned(), false, checksum(rendered.as_bytes())));
            let mut target = strict_json(wire, super::prefix_model_designs::PROPOSAL_LIMITS)
                .map_err(|_| bad())?;
            let mut replaced = 0;
            for message in target["messages"].as_array_mut().ok_or_else(bad)? {
                if message["role"] == "system" && message["content"] == document.sample_reference()
                {
                    message["content"] = json!(artifact.reference.as_ref());
                    replaced += 1;
                }
            }
            // A supported reference slot is explicit; never replace arbitrary
            // occurrences in instructions, examples, tool schemas or history.
            if replaced != 1 {
                return Err(bad());
            }
            let target_render = format!(
                "{}{}",
                artifact.prefix,
                rendered.strip_prefix(prefix).ok_or_else(bad)?
            );
            probes.push(Probe::WireChat {
                request_json: target.to_string(),
                mapping_json: mapping.into(),
                expected_model: specification.model.clone(),
                leading_prefix: artifact.prefix.to_string(),
            });
            probes.push(Probe::Text {
                text: target_render.clone(),
            });
            expected.push((id.to_owned(), true, checksum(target_render.as_bytes())));
        }
        let sdk_start = probes.len();
        let sdk_checks: Vec<_> = sdk_bodies
            .iter()
            .map(|body| json!({"id":body.id,"request_sha256":checksum(body.body.as_bytes())}))
            .collect();
        for body in &sdk_bodies {
            probes.push(Probe::WireChat {
                request_json: body.body.clone(),
                mapping_json: mapping.into(),
                expected_model: specification.model.clone(),
                leading_prefix: artifact.prefix.to_string(),
            });
        }
        let probes = serde_json::to_string(&probes).map_err(|_| bad())?;
        brassclaw_model_encoding::parse_probes(&probes).map_err(|_| bad())?;
        let reservation =
            super::final_model_encoding::EncodingReservation::reserve(governor, scope.clone())
                .map_err(|_| Error::Unavailable)?;
        let result = backend
            .encode(&b.specification, &b.artifact_set, &probes, None, scope)
            .await;
        let output = match result {
            Ok(output) => {
                reservation
                    .finish(Some(&output.usage))
                    .map_err(|_| Error::Unavailable)?;
                output.output
            }
            Err(error) => {
                reservation
                    .finish(error.usage())
                    .map_err(|_| Error::Unavailable)?;
                return Err(Error::Unavailable);
            }
        };
        let observation = strict_json(
            output.as_str().ok_or_else(bad)?,
            brassclaw_skills::value_contract::EXECUTION_DATA_LIMITS,
        )
        .map_err(|_| bad())?;
        let report: EncodingReport =
            serde_json::from_value(observation["report"].clone()).map_err(|_| bad())?;
        if observation["format"] != "brassclaw-model-encoding-observation/1"
            || observation["qualification"] != "offline-measurement-only"
            || observation["specification"] != b.specification
            || observation["artifact_set"] != b.artifact_set
            || observation["template_origin"] != "model-artifact-template"
            || observation["model_chat_template_sha256"] != document.template_sha256()
            || report.format != "brassclaw-model-encoding/3"
            || report.chat_template_sha256 != document.template_sha256()
            || !report.truncation_disabled
            || !report.padding_disabled
            || report.probes.len()
                != expected
                    .len()
                    .checked_mul(2)
                    .and_then(|n| n.checked_add(sdk_bodies.len()))
                    .ok_or_else(bad)?
        {
            return Err(bad());
        }
        let provider = super::pg_provider_repo::PgProviderRepo::new(
            self.pool.as_ref().clone(),
            &self.namespace,
        )
        .get_revision(&specification.provider_id, false)
        .await
        .map_err(|_| Error::Unavailable)?
        .ok_or_else(bad)?;
        let limits = provider
            .definition
            .model_token_limits
            .0
            .get(&specification.model)
            .ok_or_else(bad)?;
        let context = (provider.definition.default_model == specification.model)
            .then_some(provider.definition.context_window_tokens)
            .flatten()
            .ok_or_else(bad)?;
        let prefix_token_ids_sha256 = report
            .probes
            .get(sdk_start)
            .and_then(|probe| probe.partition.as_ref())
            .ok_or_else(bad)?
            .prefix_token_ids_sha256
            .clone();
        super::prefix_model_designs::hash(&prefix_token_ids_sha256)?;
        for ((_, target, render), pair) in expected.iter().zip(report.probes.chunks_exact(2)) {
            let actual = &pair[0];
            let literal = &pair[1];
            let partition = actual.partition.as_ref().ok_or_else(bad)?;
            let wire = actual.wire_input.as_ref().ok_or_else(bad)?;
            if &actual.rendered_sha256 != render
                || actual.rendered_sha256 != literal.rendered_sha256
                || actual.token_ids_sha256 != literal.token_ids_sha256
                || actual.token_count != literal.token_count
                || partition.prefix_start != 0
                || partition.prefix_tokens == 0
                || partition.prefix_tokens.checked_add(partition.prompt_tokens)
                    != Some(actual.token_count)
                || wire.model != specification.model
                || wire.mapping_sha256 != checksum(mapping.as_bytes())
            {
                return Err(bad());
            }
            if wire.reserved_output_tokens != request_output_tokens {
                return Err(bad());
            }
            if *target {
                if partition.prefix_token_ids_sha256 != prefix_token_ids_sha256
                    || partition.prefix_end != artifact.prefix.len()
                    || partition.prefix_bytes_sha256 != checksum(artifact.prefix.as_bytes())
                {
                    return Err(bad());
                }
                limits
                    .check_measured_request(
                        brassclaw_llm::ModelRequestTokenCounts {
                            prefix_tokens: partition.prefix_tokens,
                            prompt_tokens: partition.prompt_tokens,
                            input_tokens: actual.token_count,
                        },
                        u64::from(context),
                        u64::from(wire.reserved_output_tokens),
                    )
                    .map_err(|_| bad())?;
            }
        }
        for (body, actual) in sdk_bodies.iter().zip(&report.probes[sdk_start..]) {
            let partition = actual.partition.as_ref().ok_or_else(bad)?;
            let wire = actual.wire_input.as_ref().ok_or_else(bad)?;
            if partition.prefix_start != 0
                || partition.prefix_token_ids_sha256 != prefix_token_ids_sha256
                || partition.prefix_end != artifact.prefix.len()
                || partition.prefix_bytes_sha256 != checksum(artifact.prefix.as_bytes())
                || partition.prefix_tokens == 0
                || partition.prefix_tokens.checked_add(partition.prompt_tokens)
                    != Some(actual.token_count)
                || wire.request_sha256 != checksum(body.body.as_bytes())
                || wire.mapping_sha256 != checksum(mapping.as_bytes())
                || wire.model != specification.model
                || wire.reserved_output_tokens != request_output_tokens
            {
                return Err(bad());
            }
            limits
                .check_measured_request(
                    brassclaw_llm::ModelRequestTokenCounts {
                        prefix_tokens: partition.prefix_tokens,
                        prompt_tokens: partition.prompt_tokens,
                        input_tokens: actual.token_count,
                    },
                    u64::from(context),
                    u64::from(wire.reserved_output_tokens),
                )
                .map_err(|_| bad())?;
        }
        let native_hash = |name: &str| -> Result<String, Error> {
            let s = observation[name].as_str().ok_or_else(bad)?;
            super::prefix_model_designs::hash(s)?;
            Ok(s.into())
        };
        let contract = EncodingContract {
            specification: b.specification.clone(),
            artifact_set: b.artifact_set.clone(),
            provider_id: specification.provider_id,
            model: specification.model,
            pair_proposal: b.pair_proposal.clone(),
            mapping_json: mapping.into(),
            tokenizer_sha256: report.tokenizer_sha256.clone(),
            tokenizer_config_sha256: report.tokenizer_config_sha256.clone(),
            template_sha256: report.chat_template_sha256.clone(),
            worker_sha256: native_hash("worker_checksum")?,
            transport: TRANSPORT.into(),
            transport_checksum: brassclaw_llm::final_chat_contract_checksum(),
            endpoint: endpoint.to_string(),
            request_output_tokens,
        };
        let target = ReviewedTarget {
            namespace: self.namespace.clone(),
            contract,
            profile: profile.into(),
            generation: generation.into(),
            profile_proposal: b.profile_proposal.clone(),
            prefix_sha256: checksum(artifact.prefix.as_bytes()),
            prefix_token_ids_sha256,
            checks: json!({"pair_proposal_bytes":pair.proposal_bytes,"profile_proposal_bytes":content.proposal_bytes,
                "cases":expected,"native_sdk_corpus":{"format":brassclaw_llm::request_qualification::CORPUS,"requests":sdk_checks},"observation":observation,"checked_provider_revision":provider.revision,
                "operator_review_required":["reference-content-quality","deployed-provider-default-template-and-mapping","supported-request-modes"]}),
        };
        let bytes = super::prefix_generations::canonical(
            &serde_json::to_value(&target).map_err(|_| bad())?,
        )?;
        if bytes.len() > super::prefix_model_designs::PROPOSAL_LIMITS.max_bytes {
            return Err(bad());
        }
        Ok(PreparedPrefixReview {
            target,
            checksum: checksum(bytes.as_bytes()),
            bytes,
        })
    }

    async fn record(&self, id: Uuid) -> Result<Accepted, Error> {
        if id.is_nil() {
            return Err(Error::Conflict);
        }
        let client = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        let row = client.query_opt(
            "SELECT a.evidence_bytes,a.checksum,a.acceptance_scope,a.proposal_id,a.candidate_profile,a.candidate_generation,a.pair_acceptance_ref,a.profile_acceptance_ref FROM reborn_prefix_design_acceptances a JOIN reborn_prefix_design_proposals p ON p.proposal_id=a.proposal_id JOIN reborn_prefix_model_specifications s ON s.specification_id=p.specification_id WHERE a.acceptance_id=$1 AND s.provider_namespace=$2",
            &[&id,&self.namespace],
        ).await.map_err(|_| Error::Unavailable)?.ok_or(Error::Conflict)?;
        let bytes: String = row.get(0);
        if checksum(bytes.as_bytes()) != row.get::<_, String>(1) {
            return Err(Error::Unavailable);
        }
        let value = strict_json(&bytes, super::prefix_model_designs::PROPOSAL_LIMITS)
            .map_err(|_| Error::Unavailable)?;
        let accepted: Accepted = serde_json::from_value(value).map_err(|_| Error::Unavailable)?;
        let review_bytes = super::prefix_generations::canonical(
            &serde_json::to_value(&accepted.review).map_err(|_| Error::Unavailable)?,
        )?;
        let proposal = if accepted.scope == "pair" {
            &accepted.review.contract.pair_proposal
        } else {
            &accepted.review.profile_proposal
        };
        if accepted.format != FORMAT
            || accepted.id != id
            || !matches!(accepted.scope.as_str(), "pair" | "profile" | "generation")
            || accepted.scope != row.get::<_, String>(2)
            || proposal != &row.get::<_, String>(3)
            || accepted.review.profile != row.get::<_, String>(4)
            || accepted.review.generation != row.get::<_, String>(5)
            || accepted.pair_parent != row.get::<_, Option<Uuid>>(6)
            || accepted.profile_parent != row.get::<_, Option<Uuid>>(7)
            || accepted.review.namespace != self.namespace
            || accepted.review_checksum != checksum(review_bytes.as_bytes())
            || accepted.actor.trim().is_empty()
            || accepted.semantic_review.trim().is_empty()
            || accepted.review.contract.transport != TRANSPORT
            || accepted.review.contract.transport_checksum
                != brassclaw_llm::final_chat_contract_checksum()
        {
            return Err(Error::Unavailable);
        }
        Ok(accepted)
    }

    /// IDs are captured by the task's existing native selection owner. There is
    /// deliberately no lookup-latest, fallback, default prefix or approval flag.
    pub(crate) async fn resolve(
        &self,
        generation_qualification: Uuid,
        preparation_revision: u64,
    ) -> Result<Arc<FinalEncodingSelection>, Error> {
        let generation = self.record(generation_qualification).await?;
        if generation.scope != "generation" || preparation_revision == 0 {
            return Err(Error::Conflict);
        }
        let pair_id = generation.pair_parent.ok_or(Error::Conflict)?;
        let profile_id = generation.profile_parent.ok_or(Error::Conflict)?;
        let profile = self.record(profile_id).await?;
        let pair = self.record(pair_id).await?;
        let g = &generation.review;
        if pair.scope != "pair"
            || pair.pair_parent.is_some()
            || pair.profile_parent.is_some()
            || profile.scope != "profile"
            || profile.pair_parent != Some(pair_id)
            || profile.profile_parent.is_some()
            || pair.review.contract != g.contract
            || profile.review.contract != g.contract
            || profile.review.profile != g.profile
            || profile.review.profile_proposal != g.profile_proposal
        {
            return Err(Error::Conflict);
        }
        let designs = super::prefix_model_designs::PgPrefixModelDesigns::new(
            self.pool.clone(),
            self.namespace.clone(),
        );
        let specification = designs.get_specification(&g.contract.specification).await?;
        if specification.provider_id != g.contract.provider_id
            || specification.model != g.contract.model
        {
            return Err(Error::Conflict);
        }
        let proposal = designs
            .get_pair_proposal(&g.contract.specification, &g.contract.pair_proposal)
            .await?;
        let document = super::prefix_pair_construction::parse(
            &g.contract.specification,
            &proposal.proposal_bytes,
        )?;
        if document.wire_mapping_json() != Some(g.contract.mapping_json.as_str()) {
            return Err(Error::Conflict);
        }
        if document.request_output_tokens() != Some(g.contract.request_output_tokens)
            || g.contract.request_output_tokens == 0
        {
            return Err(Error::Conflict);
        }
        if designs
            .get_proposal(&g.profile, &g.profile_proposal)
            .await?
            .specification
            != g.contract.specification
        {
            return Err(Error::Conflict);
        }
        let assets = super::prefix_model_artifacts::PgPrefixModelArtifacts::new(
            self.pool.clone(),
            self.namespace.clone(),
        )
        .get(&g.contract.specification, &g.contract.artifact_set)
        .await?;
        super::prefix_pair_construction::verify_sources(&document, &assets)?;
        for (name, digest) in [
            ("tokenizer.json", &g.contract.tokenizer_sha256),
            ("tokenizer_config.json", &g.contract.tokenizer_config_sha256),
        ] {
            if !assets
                .summary
                .files
                .iter()
                .any(|f| f.name == name && &f.sha256 == digest)
            {
                return Err(Error::Conflict);
            }
        }
        let package = super::prefix_generations::PgPrefixGenerations::new(self.pool.clone())
            .get(&g.profile, &g.generation)
            .await?;
        let artifact = super::prefix_generation_binding::read(
            &g.profile,
            &g.generation,
            &package.generation_bytes,
        )?;
        artifact.require_references(
            &g.contract.specification,
            &g.contract.artifact_set,
            &g.contract.pair_proposal,
            &g.profile_proposal,
            &g.contract.mapping_json,
        )?;
        if checksum(artifact.prefix.as_bytes()) != g.prefix_sha256 {
            return Err(Error::Conflict);
        }
        Ok(Arc::new(FinalEncodingSelection {
            specification: g.contract.specification.clone(),
            artifact_set: g.contract.artifact_set.clone(),
            pair_acceptance: pair_id,
            profile_acceptance: profile_id,
            generation_qualification,
            profile: g.profile.clone(),
            generation: g.generation.clone(),
            provider_id: g.contract.provider_id.clone(),
            model: g.contract.model.clone(),
            prefix: artifact.prefix,
            prefix_token_ids_sha256: g.prefix_token_ids_sha256.clone(),
            mapping_json: Arc::from(g.contract.mapping_json.as_str()),
            tokenizer_sha256: g.contract.tokenizer_sha256.clone(),
            tokenizer_config_sha256: g.contract.tokenizer_config_sha256.clone(),
            template_sha256: g.contract.template_sha256.clone(),
            worker_sha256: g.contract.worker_sha256.clone(),
            preparation_revision,
        }))
    }
}

#[async_trait]
impl FinalEncodingBindingVerifier for PgPrefixAcceptances {
    async fn verify(
        &self,
        s: &FinalEncodingSelection,
        scope: &ResourceScope,
        headers: &reqwest::header::HeaderMap,
        body: &[u8],
    ) -> Result<(), FinalRequestError> {
        // This native transport contract supplies no prompt-affecting client
        // defaults. Authentication remains host-only. An unknown explicit header
        // cannot silently acquire template semantics.
        if scope.tenant_id.as_str() != self.namespace
            || headers
                .keys()
                .any(|k| !matches!(k.as_str(), "authorization" | "content-type"))
            || headers
                .get("content-type")
                .is_some_and(|v| v != "application/json")
        {
            return Err(FinalRequestError::Binding);
        }
        let actual = self
            .resolve(s.generation_qualification, s.preparation_revision)
            .await
            .map_err(|_| FinalRequestError::Binding)?;
        if actual.pair_acceptance != s.pair_acceptance
            || actual.profile_acceptance != s.profile_acceptance
            || actual.specification != s.specification
            || actual.artifact_set != s.artifact_set
            || actual.profile != s.profile
            || actual.generation != s.generation
            || actual.provider_id != s.provider_id
            || actual.model != s.model
            || actual.prefix != s.prefix
            || actual.prefix_token_ids_sha256 != s.prefix_token_ids_sha256
            || actual.mapping_json != s.mapping_json
            || actual.tokenizer_sha256 != s.tokenizer_sha256
            || actual.tokenizer_config_sha256 != s.tokenizer_config_sha256
            || actual.template_sha256 != s.template_sha256
            || actual.worker_sha256 != s.worker_sha256
        {
            return Err(FinalRequestError::Binding);
        }
        let accepted = self
            .record(s.generation_qualification)
            .await
            .map_err(|_| FinalRequestError::Binding)?;
        let designs = super::prefix_model_designs::PgPrefixModelDesigns::new(
            self.pool.clone(),
            self.namespace.clone(),
        );
        let proposal = designs
            .get_pair_proposal(
                &accepted.review.contract.specification,
                &accepted.review.contract.pair_proposal,
            )
            .await
            .map_err(|_| FinalRequestError::Binding)?;
        let design = super::prefix_pair_construction::parse(
            &accepted.review.contract.specification,
            &proposal.proposal_bytes,
        )
        .map_err(|_| FinalRequestError::Binding)?;
        let request = strict_json(
            std::str::from_utf8(body).map_err(|_| FinalRequestError::Body)?,
            brassclaw_skills::value_contract::EXECUTION_DATA_LIMITS,
        )
        .map_err(|_| FinalRequestError::Body)?;
        let has_tools = request["tools"]
            .as_array()
            .is_some_and(|tools| !tools.is_empty())
            || request["messages"].as_array().is_some_and(|messages| {
                messages.iter().any(|message| {
                    message["role"] == "tool"
                        || message["tool_calls"]
                            .as_array()
                            .is_some_and(|calls| !calls.is_empty())
                })
            });
        if !design.supports_mode(if has_tools { "tools" } else { "plain" }) {
            return Err(FinalRequestError::Binding);
        }
        Ok(())
    }
}
#[cfg(test)]
#[path = "prefix_acceptance_tests.rs"]
pub(crate) mod tests;
