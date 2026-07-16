//! Engine-`Store`-backed Recipe-Skill-Tool facade (Phase 7).
//!
//! Mirrors [`crate::reduction_rules_store`]: the WebUI v2's recipe /
//! tool-skill endpoints read and write the same `MemoryDoc` rows that
//! the engine's `RecipeMatcher` consults at agent-loop time. Every
//! mutation round-trips through `Store::save_memory_doc` so the
//! Postgres + libSQL backends stay consistent without a separate
//! recipe table.
//!
//! ## Read path
//!
//! `list_recipes` / `list_tool_skills` scan `list_memory_docs_with_shared`
//! for the caller's `(project_id, user_id)`, deserialize each `Recipe`
//! / `ToolSkill` from `MemoryDoc.metadata`, and project summary fields
//! into the wire `RecipeSummary` / `ToolSkillSummary` DTOs. Results are
//! sorted `updated_at DESC` so freshly-promoted validation rows
//! surface first.
//!
//! ## Write path
//!
//! `update_*_validation_status` rewrites `MemoryDoc.metadata` with the
//! new `validation_status`, an optional `review_feedback`, a
//! freshly-bumped `review_attempts`, and (for `rejected`) a
//! `rejected_at` timestamp. The DocId is unchanged — the engine's
//! `MemoryDoc` upsert key is the row's UUID, so a metadata rewrite
//! is sufficient; we don't have to touch the file system or
//! orchestrator cache.
//!
//! ## Outcome path
//!
//! `record_outcome` forwards to the engine's `MetricRecorder` so Wilson
//! lower-bound + tier classification stay consistent across both
//! surfaces (the agent loop's `RecipeStage` and the Recipe Manager UI
//! see the same numbers from one source of truth).

#![forbid(unsafe_code)]

use std::sync::Arc;

use async_trait::async_trait;
use brassclaw_engine::memory::metric_outcome::MetricRecorder;
use brassclaw_engine::traits::store::Store;
use brassclaw_engine::types::memory::{DocType, MemoryDoc};
use brassclaw_engine::types::project::ProjectId;
use brassclaw_engine::types::recipe::{
    Recipe, RecipeSource, ToolSkill, ValidationStatus,
};
use brassclaw_product_workflow::{
    OutcomeKind, RecipeDetail, RecipeKind, RecipeStore, RecipeStoreError,
    RecipeSummary, RecordOutcomeRequest, RecordOutcomeResponse, ToolSkillDetail,
    ToolSkillSummary, UpdateValidationStatusResponse,
    ValidationQueueItem, ValidationStatusValue,
};
use chrono::Utc;

/// Engine-`Store`-backed implementation of the WebUI v2
/// [`RecipeStore`] port. Holds a cheap-to-clone `Arc<dyn Store>` plus
/// a fresh `MetricRecorder` for outcome recording; both wrap the same
/// handle so a save through either path lands in the same row.
#[derive(Clone)]
pub(crate) struct StoreBackedRecipeStore {
    store: Arc<dyn Store>,
}

impl std::fmt::Debug for StoreBackedRecipeStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StoreBackedRecipeStore")
            .finish_non_exhaustive()
    }
}

impl StoreBackedRecipeStore {
    /// Open a new store backed by the engine `Store`. The supplied
    /// handle should already be the libsql/postgres-backed
    /// implementation wired into `RebornServices.memory_doc_store`.
    pub(crate) fn open(store: Arc<dyn Store>) -> Self {
        Self { store }
    }

    fn recorder(&self) -> MetricRecorder {
        MetricRecorder::new(Arc::clone(&self.store))
    }
}

#[async_trait]
impl RecipeStore for StoreBackedRecipeStore {
    async fn list_recipes(
        &self,
        user_id: &str,
        project_id: &str,
    ) -> Result<Vec<RecipeSummary>, RecipeStoreError> {
        let project_id_typed = parse_project_id(project_id)?;
        let docs = load_filtered_docs(&self.store, project_id_typed, user_id, DocType::Recipe)
            .await?;
        let mut recipes: Vec<RecipeSummary> = Vec::with_capacity(docs.len());
        for doc in docs {
            match Recipe::from_metadata(&doc.metadata) {
                Ok(recipe) => recipes.push(recipe_summary_from(&recipe)),
                Err(error) => {
                    tracing::debug!(
                        doc_id = %doc.id.0,
                        %error,
                        "recipe_store: skipping undecodable recipe"
                    );
                }
            }
        }
        recipes.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(recipes)
    }

    async fn list_tool_skills(
        &self,
        user_id: &str,
        project_id: &str,
    ) -> Result<Vec<ToolSkillSummary>, RecipeStoreError> {
        let project_id_typed = parse_project_id(project_id)?;
        let docs = load_filtered_docs(&self.store, project_id_typed, user_id, DocType::ToolSkill)
            .await?;
        let mut skills: Vec<ToolSkillSummary> = Vec::with_capacity(docs.len());
        for doc in docs {
            match ToolSkill::from_metadata(&doc.metadata) {
                Ok(skill) => skills.push(tool_skill_summary_from(&skill)),
                Err(error) => {
                    tracing::debug!(
                        doc_id = %doc.id.0,
                        %error,
                        "recipe_store: skipping undecodable tool skill"
                    );
                }
            }
        }
        skills.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(skills)
    }

    async fn get_recipe(
        &self,
        user_id: &str,
        project_id: &str,
        recipe_id: &str,
    ) -> Result<Option<RecipeDetail>, RecipeStoreError> {
        let project_id_typed = parse_project_id(project_id)?;
        let doc = find_doc(
            &self.store,
            project_id_typed,
            user_id,
            DocType::Recipe,
            recipe_id,
        )
        .await?;
        match doc {
            None => Ok(None),
            Some(doc) => {
                let recipe = Recipe::from_metadata(&doc.metadata).map_err(|e| {
                    RecipeStoreError::Invalid(format!("recipe '{recipe_id}' decode: {e}"))
                })?;
                Ok(Some(RecipeDetail {
                    id: recipe.id.clone(),
                    recipe: serde_json::to_value(&recipe).map_err(|e| {
                        RecipeStoreError::Internal(format!("recipe serialize: {e}"))
                    })?,
                }))
            }
        }
    }

    async fn get_tool_skill(
        &self,
        user_id: &str,
        project_id: &str,
        skill_id: &str,
    ) -> Result<Option<ToolSkillDetail>, RecipeStoreError> {
        let project_id_typed = parse_project_id(project_id)?;
        let doc = find_doc(
            &self.store,
            project_id_typed,
            user_id,
            DocType::ToolSkill,
            skill_id,
        )
        .await?;
        match doc {
            None => Ok(None),
            Some(doc) => {
                let skill = ToolSkill::from_metadata(&doc.metadata).map_err(|e| {
                    RecipeStoreError::Invalid(format!("skill '{skill_id}' decode: {e}"))
                })?;
                Ok(Some(ToolSkillDetail {
                    id: skill.id.clone(),
                    tool_skill: serde_json::to_value(&skill).map_err(|e| {
                        RecipeStoreError::Internal(format!("skill serialize: {e}"))
                    })?,
                }))
            }
        }
    }

    async fn list_validation_queue(
        &self,
        user_id: &str,
        project_id: &str,
    ) -> Result<Vec<ValidationQueueItem>, RecipeStoreError> {
        let project_id_typed = parse_project_id(project_id)?;
        let docs = load_filtered_docs(&self.store, project_id_typed, user_id, DocType::Recipe)
            .await?;
        let mut items: Vec<ValidationQueueItem> = Vec::new();

        for doc in docs {
            let recipe = match Recipe::from_metadata(&doc.metadata) {
                Ok(r) => r,
                Err(_) => continue,
            };
            if is_queue_status(&recipe.validation_status) {
                items.push(validation_item_for_recipe(&recipe, RecipeKind::Recipe));
            }
        }

        let docs = load_filtered_docs(&self.store, project_id_typed, user_id, DocType::ToolSkill)
            .await?;
        for doc in docs {
            let skill = match ToolSkill::from_metadata(&doc.metadata) {
                Ok(s) => s,
                Err(_) => continue,
            };
            if is_queue_status(&skill.validation_status) {
                items.push(validation_item_for_skill(&skill));
            }
        }

        items.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        Ok(items)
    }

    async fn count_by_status(
        &self,
        user_id: &str,
        project_id: &str,
        status: &str,
    ) -> Result<u32, RecipeStoreError> {
        let project_id_typed = parse_project_id(project_id)?;
        let mut count: u32 = 0;
        for kind in [DocType::Recipe, DocType::ToolSkill] {
            let docs = load_filtered_docs(&self.store, project_id_typed, user_id, kind)
                .await?;
            for doc in docs {
                let value = match kind {
                    DocType::Recipe => {
                        Recipe::from_metadata(&doc.metadata).ok().map(|r| r.validation_status)
                    }
                    DocType::ToolSkill => {
                        ToolSkill::from_metadata(&doc.metadata).ok().map(|s| s.validation_status)
                    }
                    _ => None,
                };
                if let Some(actual) = value
                    && status_label(actual) == status
                {
                    count = count.saturating_add(1);
                }
            }
        }
        Ok(count)
    }

    async fn update_recipe_validation_status(
        &self,
        user_id: &str,
        project_id: &str,
        recipe_id: &str,
        new_status: &str,
        feedback: Option<&str>,
    ) -> Result<UpdateValidationStatusResponse, RecipeStoreError> {
        let project_id_typed = parse_project_id(project_id)?;
        let doc = find_own_doc(
            &self.store,
            project_id_typed,
            user_id,
            DocType::Recipe,
            recipe_id,
        )
        .await?
        .ok_or_else(|| RecipeStoreError::NotFound(format!("recipe '{recipe_id}'")))?;
        let mut recipe = Recipe::from_metadata(&doc.metadata).map_err(|e| {
            RecipeStoreError::Invalid(format!("recipe '{recipe_id}' decode: {e}"))
        })?;
        let previous_status = status_label(recipe.validation_status.clone());
        let target = parse_status(new_status)?;
        if !is_valid_transition(&recipe.validation_status, &target) {
            return Err(RecipeStoreError::Invalid(format!(
                "invalid status transition '{previous_status}' → '{new_status}' for recipe '{recipe_id}'"
            )));
        }
        if matches!(target, ValidationStatus::Rejected) {
            recipe.rejected_at = Some(Utc::now());
        }
        recipe.validation_status = target.clone();
        if let Some(text) = feedback {
            recipe.review_feedback = Some(text.to_string());
        } else if matches!(target, ValidationStatus::Validated) {
            recipe.review_feedback = None;
        }
        recipe.updated_at = Utc::now();
        let mut updated_doc = doc.clone();
        updated_doc.metadata = recipe.to_metadata().map_err(|e| {
            RecipeStoreError::Invalid(format!("recipe '{recipe_id}' encode: {e}"))
        })?;
        updated_doc.updated_at = recipe.updated_at;
        self.store.save_memory_doc(&updated_doc).await.map_err(|e| {
            RecipeStoreError::Unavailable(format!("Store::save_memory_doc recipe: {e}"))
        })?;
        tracing::debug!(
            user_id,
            recipe_id,
            previous_status = %previous_status,
            new_status = %new_status,
            review_attempts = recipe.review_attempts,
            "recipe_store: updated recipe validation status"
        );
        Ok(UpdateValidationStatusResponse {
            id: recipe.id.clone(),
            item_type: RecipeKind::Recipe,
            previous_status,
            new_status: new_status.to_string(),
            review_attempts: recipe.review_attempts,
        })
    }

    async fn update_skill_validation_status(
        &self,
        user_id: &str,
        project_id: &str,
        skill_id: &str,
        new_status: &str,
        feedback: Option<&str>,
    ) -> Result<UpdateValidationStatusResponse, RecipeStoreError> {
        let project_id_typed = parse_project_id(project_id)?;
        let doc = find_own_doc(
            &self.store,
            project_id_typed,
            user_id,
            DocType::ToolSkill,
            skill_id,
        )
        .await?
        .ok_or_else(|| RecipeStoreError::NotFound(format!("skill '{skill_id}'")))?;
        let mut skill = ToolSkill::from_metadata(&doc.metadata).map_err(|e| {
            RecipeStoreError::Invalid(format!("skill '{skill_id}' decode: {e}"))
        })?;
        let previous_status = status_label(skill.validation_status.clone());
        let target = parse_status(new_status)?;
        if !is_valid_transition(&skill.validation_status, &target) {
            return Err(RecipeStoreError::Invalid(format!(
                "invalid status transition '{previous_status}' → '{new_status}' for skill '{skill_id}'"
            )));
        }
        if matches!(target, ValidationStatus::Rejected) {
            skill.rejected_at = Some(Utc::now());
        }
        skill.validation_status = target.clone();
        if let Some(text) = feedback {
            skill.review_feedback = Some(text.to_string());
        } else if matches!(target, ValidationStatus::Validated) {
            skill.review_feedback = None;
        }
        skill.updated_at = Utc::now();
        let mut updated_doc = doc.clone();
        updated_doc.metadata = skill.to_metadata().map_err(|e| {
            RecipeStoreError::Invalid(format!("skill '{skill_id}' encode: {e}"))
        })?;
        updated_doc.updated_at = skill.updated_at;
        self.store.save_memory_doc(&updated_doc).await.map_err(|e| {
            RecipeStoreError::Unavailable(format!("Store::save_memory_doc skill: {e}"))
        })?;
        tracing::debug!(
            user_id,
            skill_id,
            previous_status = %previous_status,
            new_status = %new_status,
            review_attempts = skill.review_attempts,
            "recipe_store: updated tool skill validation status"
        );
        Ok(UpdateValidationStatusResponse {
            id: skill.id.clone(),
            item_type: RecipeKind::ToolSkill,
            previous_status,
            new_status: new_status.to_string(),
            review_attempts: skill.review_attempts,
        })
    }

    async fn record_outcome(
        &self,
        user_id: &str,
        project_id: &str,
        request: RecordOutcomeRequest,
    ) -> Result<RecordOutcomeResponse, RecipeStoreError> {
        let project_id_typed = parse_project_id(project_id)?;
        let recorder = self.recorder();
        let recorded = match request.kind {
            OutcomeKind::Recipe => recorder
                .record_recipe(project_id_typed, user_id, &request.id, request.success)
                .await
                .map_err(|e| RecipeStoreError::Unavailable(format!("MetricRecorder: {e}")))?,
            OutcomeKind::ToolSkill => recorder
                .record_tool_skill(project_id_typed, user_id, &request.id, request.success)
                .await
                .map_err(|e| RecipeStoreError::Unavailable(format!("MetricRecorder: {e}")))?,
        };
        let _ = recorded;
        Ok(RecordOutcomeResponse {
            id: request.id,
            kind: request.kind,
            recorded: true,
        })
    }
}

// --- DTO projection helpers ----------------------------------------

fn recipe_summary_from(recipe: &Recipe) -> RecipeSummary {
    RecipeSummary {
        id: recipe.id.clone(),
        name: recipe.name.clone(),
        description: recipe.description.clone(),
        category: recipe.category.clone(),
        trigger: serde_json::to_value(&recipe.trigger).unwrap_or(serde_json::Value::Null),
        step_count: recipe.steps.len() as u32,
        usage_count: recipe.usage_count,
        success_count: recipe.success_count,
        failure_count: recipe.failure_count,
        wilson_lower: recipe.wilson_lower,
        tier: recipe.tier.clone(),
        tier0_eligible: recipe.is_tier0_eligible(),
        validation_status: status_label(recipe.validation_status.clone()),
        validation_errors: recipe.validation_errors.clone(),
        review_attempts: recipe.review_attempts,
        source: source_label(&recipe.source),
        created_at: recipe.created_at.to_rfc3339(),
        updated_at: recipe.updated_at.to_rfc3339(),
    }
}

fn tool_skill_summary_from(skill: &ToolSkill) -> ToolSkillSummary {
    ToolSkillSummary {
        id: skill.id.clone(),
        name: skill.name.clone(),
        tool_name: skill.tool_name.clone(),
        description: skill.description.clone(),
        category: skill.category.clone(),
        estimated_tokens: skill.estimated_tokens() as u32,
        usage_count: skill.usage_count,
        success_count: skill.success_count,
        failure_count: skill.failure_count,
        wilson_lower: skill.wilson_lower,
        tier: skill.tier.clone(),
        validation_status: status_label(skill.validation_status.clone()),
        validation_errors: skill.validation_errors.clone(),
        review_attempts: skill.review_attempts,
        source: source_label(&skill.source),
        created_at: skill.created_at.to_rfc3339(),
        updated_at: skill.updated_at.to_rfc3339(),
    }
}

fn validation_item_for_recipe(
    recipe: &Recipe,
    kind: RecipeKind,
) -> ValidationQueueItem {
    ValidationQueueItem {
        id: recipe.id.clone(),
        name: recipe.name.clone(),
        item_type: kind,
        category: recipe.category.clone(),
        description: recipe.description.clone(),
        trigger_summary: recipe.trigger.signature(),
        estimated_tokens: None,
        validation_status: status_label(recipe.validation_status.clone()),
        validation_errors: recipe.validation_errors.clone(),
        review_feedback: recipe.review_feedback.clone(),
        review_attempts: recipe.review_attempts,
        similarity_parent_id: recipe.similarity_parent_id.clone(),
        created_at: recipe.created_at.to_rfc3339(),
        source: source_label(&recipe.source),
    }
}

fn validation_item_for_skill(skill: &ToolSkill) -> ValidationQueueItem {
    ValidationQueueItem {
        id: skill.id.clone(),
        name: skill.name.clone(),
        item_type: RecipeKind::ToolSkill,
        category: skill.category.clone(),
        description: skill.description.clone(),
        trigger_summary: skill.tool_name.clone(),
        estimated_tokens: Some(skill.estimated_tokens() as u32),
        validation_status: status_label(skill.validation_status.clone()),
        validation_errors: skill.validation_errors.clone(),
        review_feedback: skill.review_feedback.clone(),
        review_attempts: skill.review_attempts,
        similarity_parent_id: skill.similarity_parent_id.clone(),
        created_at: skill.created_at.to_rfc3339(),
        source: source_label(&skill.source),
    }
}

fn source_label(source: &RecipeSource) -> String {
    match source {
        RecipeSource::Extracted => "extracted".to_string(),
        RecipeSource::Authored => "authored".to_string(),
        RecipeSource::Imported => "imported".to_string(),
    }
}

fn status_label(status: ValidationStatus) -> ValidationStatusValue {
    match status {
        ValidationStatus::Pending => "pending".to_string(),
        ValidationStatus::UpgradeQueued => "upgrade_queued".to_string(),
        ValidationStatus::AutoFailed => "auto_failed".to_string(),
        ValidationStatus::AutoPassed => "auto_passed".to_string(),
        ValidationStatus::Validated => "validated".to_string(),
        ValidationStatus::ReviewRequested => "review_requested".to_string(),
        ValidationStatus::Rejected => "rejected".to_string(),
        ValidationStatus::Garbage => "garbage".to_string(),
    }
}

fn parse_status(raw: &str) -> Result<ValidationStatus, RecipeStoreError> {
    match raw {
        "pending" => Ok(ValidationStatus::Pending),
        "upgrade_queued" => Ok(ValidationStatus::UpgradeQueued),
        "auto_failed" => Ok(ValidationStatus::AutoFailed),
        "auto_passed" => Ok(ValidationStatus::AutoPassed),
        "validated" => Ok(ValidationStatus::Validated),
        "review_requested" => Ok(ValidationStatus::ReviewRequested),
        "rejected" => Ok(ValidationStatus::Rejected),
        "garbage" => Ok(ValidationStatus::Garbage),
        other => Err(RecipeStoreError::Invalid(format!(
            "unknown validation_status '{other}'"
        ))),
    }
}

fn is_queue_status(status: &ValidationStatus) -> bool {
    matches!(
        status,
        ValidationStatus::AutoPassed
            | ValidationStatus::ReviewRequested
            | ValidationStatus::UpgradeQueued
    )
}

/// Guard user-triggered validation state transitions. Only the transitions
/// reachable via WebUI validation-queue actions are permitted; all others
/// (e.g. `Pending → Validated`, bypassing auto-validation; `Rejected → Validated`,
/// bypassing the rejection lifecycle) are rejected with `RecipeStoreError::Invalid`.
///
/// The automated review service (`RecipeReviewService`) mutates status via
/// the engine `Store` directly and is NOT subject to this guard.
fn is_valid_transition(from: &ValidationStatus, to: &ValidationStatus) -> bool {
    matches!(
        (from, to),
        (ValidationStatus::AutoPassed, ValidationStatus::Validated)
            | (ValidationStatus::AutoPassed, ValidationStatus::Rejected)
            | (ValidationStatus::AutoPassed, ValidationStatus::ReviewRequested)
            | (ValidationStatus::ReviewRequested, ValidationStatus::Rejected)
            | (ValidationStatus::UpgradeQueued, ValidationStatus::Rejected)
    )
}

// --- Store bridge --------------------------------------------------

async fn load_filtered_docs(
    store: &Arc<dyn Store>,
    project_id: ProjectId,
    user_id: &str,
    doc_type: DocType,
) -> Result<Vec<MemoryDoc>, RecipeStoreError> {
    store
        .list_memory_docs_with_shared(project_id, user_id)
        .await
        .map(|docs| docs.into_iter().filter(|d| d.doc_type == doc_type).collect())
        .map_err(|e| RecipeStoreError::Unavailable(format!("Store::list_memory_docs_with_shared: {e}")))
}

async fn find_doc(
    store: &Arc<dyn Store>,
    project_id: ProjectId,
    user_id: &str,
    doc_type: DocType,
    target_id: &str,
) -> Result<Option<MemoryDoc>, RecipeStoreError> {
    let docs = load_filtered_docs(store, project_id, user_id, doc_type).await?;
    for doc in docs {
        let metadata_id = doc
            .metadata
            .get("id")
            .and_then(|value| value.as_str())
            .map(str::to_string);
        if metadata_id.as_deref() == Some(target_id) {
            return Ok(Some(doc));
        }
    }
    Ok(None)
}

/// Like `find_doc` but searches only the caller's own docs (via
/// `Store::list_memory_docs`, NOT `list_memory_docs_with_shared`).
/// Used by write operations so shared/admin recipes are never
/// accessible for mutation by non-owner users.
async fn find_own_doc(
    store: &Arc<dyn Store>,
    project_id: ProjectId,
    user_id: &str,
    doc_type: DocType,
    target_id: &str,
) -> Result<Option<MemoryDoc>, RecipeStoreError> {
    let docs = store
        .list_memory_docs(project_id, user_id)
        .await
        .map(|docs| docs.into_iter().filter(|d| d.doc_type == doc_type).collect::<Vec<_>>())
        .map_err(|e| RecipeStoreError::Unavailable(format!("Store::list_memory_docs: {e}")))?;
    for doc in docs {
        let metadata_id = doc
            .metadata
            .get("id")
            .and_then(|value| value.as_str())
            .map(str::to_string);
        if metadata_id.as_deref() == Some(target_id) {
            return Ok(Some(doc));
        }
    }
    Ok(None)
}

/// Parse a wire `project_id` into a typed `ProjectId`. Mirrors the
/// rules in `reduction_rules_store::parse_project_id`. The wire slug
/// is treated as the `slug` parameter (recoverable from the wire)
/// while `tenant_id` is fixed so every persistent row in the operator
/// surface lives under the same tenant. This keeps `(project_id,
/// user_id)` keys consistent across `RecipeStore` and the engine's
/// orchestrator cache.
fn parse_project_id(raw: &str) -> Result<ProjectId, RecipeStoreError> {
    if raw.is_empty() {
        return Err(RecipeStoreError::Invalid(
            "project_id is empty".to_string(),
        ));
    }
    if raw.len() > 64 {
        return Err(RecipeStoreError::Invalid(format!(
            "project_id too long: {} chars",
            raw.len()
        )));
    }
    if !raw
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(RecipeStoreError::Invalid(format!(
            "project_id '{raw}' contains invalid characters"
        )));
    }
    Ok(ProjectId::from_slug("brassclaw-recipe-library", raw))
}

#[cfg(test)]
mod tests {
    use super::*;
    use brassclaw_engine::traits::store::Store;
    use brassclaw_engine::types::capability::{CapabilityLease, LeaseId};
    use brassclaw_engine::types::error::EngineError;
    use brassclaw_engine::types::event::ThreadEvent;
    use brassclaw_engine::types::memory::DocId;
    use brassclaw_engine::types::mission::{Mission, MissionId, MissionStatus};
    use brassclaw_engine::types::recipe::{
        RecipeStep, RecipeTrigger, RecipeValidation, ToolSkillParam,
    };
    use brassclaw_engine::types::step::Step;
    use brassclaw_engine::types::thread::{Thread, ThreadId, ThreadState};

    /// Minimal `Store` impl used only by these tests. Implements the
    /// `MemoryDoc` + thread/step/mission surfaces the recipe composite
    /// path actually touches; everything else panics with
    /// `unimplemented!` so an accidental call reveals the test gap.
    #[derive(Default)]
    struct InMemoryEngineStore {
        docs: tokio::sync::RwLock<Vec<MemoryDoc>>,
    }

    impl InMemoryEngineStore {
        async fn add(&self, doc: MemoryDoc) {
            self.docs.write().await.push(doc);
        }
        fn matches_project_user(docs: &[MemoryDoc], project_id: ProjectId, user_id: &str) -> Vec<MemoryDoc> {
            docs.iter()
                .filter(|d| d.project_id == project_id && d.user_id == user_id)
                .cloned()
                .collect()
        }
    }

    #[async_trait::async_trait]
    impl Store for InMemoryEngineStore {
        async fn save_thread(&self, _thread: &Thread) -> Result<(), EngineError> {
            unimplemented!("InMemoryEngineStore is recipe-test-only scope")
        }
        async fn load_thread(
            &self,
            _id: ThreadId,
        ) -> Result<Option<Thread>, EngineError> {
            unimplemented!("InMemoryEngineStore is recipe-test-only scope")
        }
        async fn list_threads(
            &self,
            _project_id: ProjectId,
            _user_id: &str,
        ) -> Result<Vec<Thread>, EngineError> {
            unimplemented!("InMemoryEngineStore is recipe-test-only scope")
        }
        async fn update_thread_state(
            &self,
            _id: ThreadId,
            _state: ThreadState,
        ) -> Result<(), EngineError> {
            unimplemented!("InMemoryEngineStore is recipe-test-only scope")
        }
        async fn save_step(&self, _step: &Step) -> Result<(), EngineError> {
            unimplemented!("InMemoryEngineStore is recipe-test-only scope")
        }
        async fn load_steps(
            &self,
            _thread_id: ThreadId,
        ) -> Result<Vec<Step>, EngineError> {
            unimplemented!("InMemoryEngineStore is recipe-test-only scope")
        }
        async fn append_events(
            &self,
            _events: &[ThreadEvent],
        ) -> Result<(), EngineError> {
            unimplemented!("InMemoryEngineStore is recipe-test-only scope")
        }
        async fn load_events(
            &self,
            _thread_id: ThreadId,
        ) -> Result<Vec<ThreadEvent>, EngineError> {
            unimplemented!("InMemoryEngineStore is recipe-test-only scope")
        }
        async fn save_project(
            &self,
            _project: &brassclaw_engine::types::project::Project,
        ) -> Result<(), EngineError> {
            unimplemented!("InMemoryEngineStore is recipe-test-only scope")
        }
        async fn load_project(
            &self,
            _id: ProjectId,
        ) -> Result<Option<brassclaw_engine::types::project::Project>, EngineError> {
            unimplemented!("InMemoryEngineStore is recipe-test-only scope")
        }
        async fn save_memory_doc(&self, doc: &MemoryDoc) -> Result<(), EngineError> {
            let mut docs = self.docs.write().await;
            docs.retain(|d| d.id != doc.id);
            docs.push(doc.clone());
            Ok(())
        }
        async fn load_memory_doc(
            &self,
            id: DocId,
        ) -> Result<Option<MemoryDoc>, EngineError> {
            Ok(self.docs.read().await.iter().find(|d| d.id == id).cloned())
        }
        async fn list_memory_docs(
            &self,
            project_id: ProjectId,
            user_id: &str,
        ) -> Result<Vec<MemoryDoc>, EngineError> {
            Ok(Self::matches_project_user(
                &self.docs.read().await,
                project_id,
                user_id,
            ))
        }
        async fn save_lease(
            &self,
            _lease: &CapabilityLease,
        ) -> Result<(), EngineError> {
            unimplemented!("InMemoryEngineStore is recipe-test-only scope")
        }
        async fn load_active_leases(
            &self,
            _thread_id: ThreadId,
        ) -> Result<Vec<CapabilityLease>, EngineError> {
            unimplemented!("InMemoryEngineStore is recipe-test-only scope")
        }
        async fn revoke_lease(
            &self,
            _lease_id: LeaseId,
            _reason: &str,
        ) -> Result<(), EngineError> {
            unimplemented!("InMemoryEngineStore is recipe-test-only scope")
        }
        async fn save_mission(
            &self,
            _mission: &Mission,
        ) -> Result<(), EngineError> {
            unimplemented!("InMemoryEngineStore is recipe-test-only scope")
        }
        async fn load_mission(
            &self,
            _id: MissionId,
        ) -> Result<Option<Mission>, EngineError> {
            unimplemented!("InMemoryEngineStore is recipe-test-only scope")
        }
        async fn list_missions(
            &self,
            _project_id: ProjectId,
            _user_id: &str,
        ) -> Result<Vec<Mission>, EngineError> {
            unimplemented!("InMemoryEngineStore is recipe-test-only scope")
        }
        async fn update_mission_status(
            &self,
            _id: MissionId,
            _status: MissionStatus,
        ) -> Result<(), EngineError> {
            unimplemented!("InMemoryEngineStore is recipe-test-only scope")
        }
    }

    fn make_pair() -> (Arc<InMemoryEngineStore>, Arc<dyn Store>) {
        let typed = Arc::new(InMemoryEngineStore::default());
        let erased: Arc<dyn Store> = Arc::clone(&typed) as Arc<dyn Store>;
        (typed, erased)
    }

    fn sample_recipe(id: &str, status: ValidationStatus) -> Recipe {
        let now = Utc::now();
        Recipe {
            id: id.to_string(),
            name: id.to_string(),
            description: format!("sample recipe {id}"),
            trigger: RecipeTrigger::Keyword {
                keywords: vec!["deploy".to_string(), "release".to_string()],
                threshold: 0.5,
            },
            steps: vec![RecipeStep {
                skill: "git-status".to_string(),
                tool: "shell".to_string(),
                params: serde_json::json!({"cmd": "git status"}),
                description: "git-status check".to_string(),
            }],
            validation: RecipeValidation::None,
            category: "deploy".to_string(),
            usage_count: 0,
            success_count: 0,
            failure_count: 0,
            wilson_lower: 0.0,
            tier: "seedling".to_string(),
            source: RecipeSource::Extracted,
            source_thread_id: None,
            project_id: "bootstrap".to_string(),
            user_id: "user1".to_string(),
            validation_status: status,
            validation_errors: Vec::new(),
            review_feedback: None,
            review_attempts: 0,
            rejected_at: None,
            similarity_parent_id: None,
            skip_similarity: false,
            last_audit_at: None,
            audit_failure_count: 0,
            replaces_id: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn sample_skill(id: &str, status: ValidationStatus) -> ToolSkill {
        let now = Utc::now();
        ToolSkill {
            id: id.to_string(),
            name: id.to_string(),
            tool_name: "shell".to_string(),
            description: format!("sample skill {id}"),
            param_template: serde_json::json!({"cmd": "ls"}),
            param_schema: vec![ToolSkillParam {
                name: "cmd".to_string(),
                param_type: "string".to_string(),
                description: "shell command".to_string(),
                required: true,
            }],
            preconditions: "shell present".to_string(),
            error_handling: "report stderr".to_string(),
            code_snippet: Some("process(cmd)".to_string()),
            category: "shell".to_string(),
            usage_count: 0,
            success_count: 0,
            failure_count: 0,
            wilson_lower: 0.0,
            tier: "seedling".to_string(),
            source: RecipeSource::Extracted,
            source_thread_id: None,
            project_id: "bootstrap".to_string(),
            user_id: "user1".to_string(),
            validation_status: status,
            validation_errors: Vec::new(),
            review_feedback: None,
            review_attempts: 0,
            rejected_at: None,
            similarity_parent_id: None,
            skip_similarity: false,
            last_audit_at: None,
            audit_failure_count: 0,
            replaces_id: None,
            created_at: now,
            updated_at: now,
        }
    }

    async fn save_recipe_doc(typed: &InMemoryEngineStore, project_id: ProjectId, recipe: &Recipe) {
        let metadata = recipe.to_metadata().expect("recipe metadata encode");
        typed
            .add(MemoryDoc {
                id: DocId::new(),
                project_id,
                user_id: recipe.user_id.clone(),
                doc_type: DocType::Recipe,
                title: recipe.name.clone(),
                content: serde_json::to_string(&metadata).unwrap_or_default(),
                source_thread_id: None,
                tags: vec!["recipe".to_string()],
                metadata,
                created_at: recipe.created_at,
                updated_at: recipe.updated_at,
            })
            .await;
    }

    async fn save_skill_doc(typed: &InMemoryEngineStore, project_id: ProjectId, skill: &ToolSkill) {
        let metadata = skill.to_metadata().expect("skill metadata encode");
        typed
            .add(MemoryDoc {
                id: DocId::new(),
                project_id,
                user_id: skill.user_id.clone(),
                doc_type: DocType::ToolSkill,
                title: skill.name.clone(),
                content: serde_json::to_string(&metadata).unwrap_or_default(),
                source_thread_id: None,
                tags: vec!["tool_skill".to_string()],
                metadata,
                created_at: skill.created_at,
                updated_at: skill.updated_at,
            })
            .await;
    }

    fn project_id(slug: &str) -> ProjectId {
        ProjectId::from_slug("brassclaw-recipe-library", slug)
    }

    #[tokio::test]
    async fn list_recipes_returns_empty_when_store_empty() {
        let (_typed, erased) = make_pair();
        let store = StoreBackedRecipeStore::open(erased);
        let result = store.list_recipes("user1", "bootstrap").await.expect("list");
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn list_recipes_returns_only_recipe_docs() {
        let (typed, erased) = make_pair();
        let project = project_id("bootstrap");
        let r = sample_recipe("r1", ValidationStatus::Validated);
        let s = sample_skill("s1", ValidationStatus::Validated);
        save_recipe_doc(&typed, project, &r).await;
        save_skill_doc(&typed, project, &s).await;
        let store = StoreBackedRecipeStore::open(erased);
        let recipes = store.list_recipes("user1", "bootstrap").await.expect("list");
        assert_eq!(recipes.len(), 1);
        assert_eq!(recipes[0].id, "r1");
        let skills = store.list_tool_skills("user1", "bootstrap").await.expect("list");
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].id, "s1");
    }

    #[tokio::test]
    async fn list_recipes_ignores_cross_tenant_rows() {
        let (typed, erased) = make_pair();
        let project = project_id("alpha");
        let r = sample_recipe("alpha-recipe", ValidationStatus::Validated);
        save_recipe_doc(&typed, project, &r).await;
        let store = StoreBackedRecipeStore::open(erased);
        let beta = store.list_recipes("user1", "beta").await.expect("list beta");
        assert!(beta.is_empty(), "alpha recipe must not leak into beta");
        let alpha = store.list_recipes("user1", "alpha").await.expect("list alpha");
        assert_eq!(alpha.len(), 1);
        assert_eq!(alpha[0].id, "alpha-recipe");
    }

    #[tokio::test]
    async fn get_recipe_returns_none_for_unknown_id() {
        let (_typed, erased) = make_pair();
        let store = StoreBackedRecipeStore::open(erased);
        let result = store
            .get_recipe("user1", "bootstrap", "missing")
            .await
            .expect("get");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn get_recipe_round_trips_metadata() {
        let (typed, erased) = make_pair();
        let project = project_id("bootstrap");
        let r = sample_recipe("r1", ValidationStatus::Validated);
        save_recipe_doc(&typed, project, &r).await;
        let store = StoreBackedRecipeStore::open(erased);
        let result = store
            .get_recipe("user1", "bootstrap", "r1")
            .await
            .expect("get")
            .expect("some");
        assert_eq!(result.id, "r1");
        let json = result.recipe;
        assert_eq!(json["name"], "r1");
        assert_eq!(json["trigger"]["type"], "keyword");
        let kw = json["trigger"]["keywords"].as_array().expect("keywords array");
        let kw_vec: Vec<String> = kw
            .iter()
            .map(|v| v.as_str().unwrap_or("").to_string())
            .collect();
        assert!(
            kw_vec.contains(&"deploy".to_string())
                && kw_vec.contains(&"release".to_string()),
            "keywords must round-trip through to_metadata/from_metadata"
        );
    }

    #[tokio::test]
    async fn validation_queue_only_returns_queue_status_items() {
        let (typed, erased) = make_pair();
        let project = project_id("bootstrap");
        let mut auto_passed = sample_recipe("auto_passed", ValidationStatus::AutoPassed);
        let pending = sample_recipe("pending", ValidationStatus::Pending);
        let auto_failed = sample_recipe("auto_failed", ValidationStatus::AutoFailed);
        let validated = sample_recipe("validated", ValidationStatus::Validated);
        let rejected = sample_recipe("rejected", ValidationStatus::Rejected);
        let garbage = sample_recipe("garbage", ValidationStatus::Garbage);
        let upgrade = sample_recipe("upgrade", ValidationStatus::UpgradeQueued);
        let skill_auto_passed = sample_skill("skill-auto-passed", ValidationStatus::AutoPassed);
        let skill_validated = sample_skill("skill-validated", ValidationStatus::Validated);
        // auto_passed recipe created slightly earlier so the
        // created_at-ascending sort predicts it first.
        auto_passed.created_at = Utc::now() - chrono::Duration::seconds(5);
        auto_passed.updated_at = auto_passed.created_at;
        save_recipe_doc(&typed, project, &auto_passed).await;
        save_recipe_doc(&typed, project, &pending).await;
        save_recipe_doc(&typed, project, &auto_failed).await;
        save_recipe_doc(&typed, project, &validated).await;
        save_recipe_doc(&typed, project, &rejected).await;
        save_recipe_doc(&typed, project, &garbage).await;
        save_recipe_doc(&typed, project, &upgrade).await;
        save_skill_doc(&typed, project, &skill_auto_passed).await;
        save_skill_doc(&typed, project, &skill_validated).await;
        let store = StoreBackedRecipeStore::open(erased);
        let items = store
            .list_validation_queue("user1", "bootstrap")
            .await
            .expect("list");
        // AutoPassed + UpgradeQueued recipes appear; AutoPassed skill appears.
        // Pending and AutoFailed are automated-pipeline statuses — not shown
        // to users. Validated/Rejected/Garbage are also excluded.
        let ids: Vec<String> = items.iter().map(|i| i.id.clone()).collect();
        assert!(ids.contains(&"auto_passed".to_string()));
        assert!(ids.contains(&"upgrade".to_string()));
        assert!(ids.contains(&"skill-auto-passed".to_string()));
        assert!(!ids.contains(&"pending".to_string()));
        assert!(!ids.contains(&"auto_failed".to_string()));
        assert!(!ids.contains(&"validated".to_string()));
        assert!(!ids.contains(&"rejected".to_string()));
        assert!(!ids.contains(&"garbage".to_string()));
        assert!(!ids.contains(&"skill-validated".to_string()));
        assert_eq!(items.len(), 3);
    }

    #[tokio::test]
    async fn count_by_status_matches_filter() {
        let (typed, erased) = make_pair();
        let project = project_id("bootstrap");
        let r1 = sample_recipe("r1", ValidationStatus::Validated);
        let r2 = sample_recipe("r2", ValidationStatus::Validated);
        let r3 = sample_recipe("r3", ValidationStatus::Pending);
        let s1 = sample_skill("s1", ValidationStatus::Validated);
        save_recipe_doc(&typed, project, &r1).await;
        save_recipe_doc(&typed, project, &r2).await;
        save_recipe_doc(&typed, project, &r3).await;
        save_skill_doc(&typed, project, &s1).await;
        let store = StoreBackedRecipeStore::open(erased);
        let validated = store
            .count_by_status("user1", "bootstrap", "validated")
            .await
            .expect("count");
        assert_eq!(validated, 3, "two validated recipes + one validated skill");
        let pending = store
            .count_by_status("user1", "bootstrap", "pending")
            .await
            .expect("count");
        assert_eq!(pending, 1);
        let rejected = store
            .count_by_status("user1", "bootstrap", "rejected")
            .await
            .expect("count");
        assert_eq!(rejected, 0);
    }

    #[tokio::test]
    async fn update_recipe_validation_status_moves_through_states() {
        let (typed, erased) = make_pair();
        let project = project_id("bootstrap");
        let r = sample_recipe("r1", ValidationStatus::AutoPassed);
        save_recipe_doc(&typed, project, &r).await;
        let store = StoreBackedRecipeStore::open(erased);

        // auto_passed → review_requested (with feedback)
        // review_attempts must NOT be bumped by the user action (only
        // RecipeReviewService increments this counter).
        let resp = store
            .update_recipe_validation_status(
                "user1",
                "bootstrap",
                "r1",
                "review_requested",
                Some("please fix step 1"),
            )
            .await
            .expect("review");
        assert_eq!(resp.previous_status, "auto_passed");
        assert_eq!(resp.new_status, "review_requested");
        assert_eq!(resp.review_attempts, 0, "user action must not bump review_attempts");

        let current = store
            .get_recipe("user1", "bootstrap", "r1")
            .await
            .expect("get")
            .expect("some");
        assert_eq!(current.recipe["validation_status"], "review_requested");
        assert_eq!(current.recipe["review_feedback"], "please fix step 1");

        // review_requested → rejected sets rejected_at
        let resp = store
            .update_recipe_validation_status("user1", "bootstrap", "r1", "rejected", None)
            .await
            .expect("reject");
        assert_eq!(resp.previous_status, "review_requested");
        assert_eq!(resp.new_status, "rejected");

        let current = store
            .get_recipe("user1", "bootstrap", "r1")
            .await
            .expect("get")
            .expect("some");
        assert_eq!(current.recipe["validation_status"], "rejected");
        assert!(
            current.recipe["rejected_at"].is_string(),
            "rejected_at timestamp must be set on rejection"
        );
    }

    #[tokio::test]
    async fn update_recipe_validation_status_blocks_invalid_transitions() {
        let (typed, erased) = make_pair();
        let project = project_id("bootstrap");
        let pending = sample_recipe("pending", ValidationStatus::Pending);
        let rejected = sample_recipe("rejected", ValidationStatus::Rejected);
        save_recipe_doc(&typed, project, &pending).await;
        save_recipe_doc(&typed, project, &rejected).await;
        let store = StoreBackedRecipeStore::open(erased);

        // Pending → Validated bypasses auto-validation — must be blocked.
        let result = store
            .update_recipe_validation_status("user1", "bootstrap", "pending", "validated", None)
            .await;
        assert!(
            matches!(result, Err(RecipeStoreError::Invalid(_))),
            "Pending → Validated must be an invalid transition"
        );

        // Rejected → Validated bypasses the rejection lifecycle — must be blocked.
        let result = store
            .update_recipe_validation_status("user1", "bootstrap", "rejected", "validated", None)
            .await;
        assert!(
            matches!(result, Err(RecipeStoreError::Invalid(_))),
            "Rejected → Validated must be an invalid transition"
        );
    }

    #[tokio::test]
    async fn update_recipe_validation_status_unknown_id_returns_not_found() {
        let (_typed, erased) = make_pair();
        let store = StoreBackedRecipeStore::open(erased);
        let result = store
            .update_recipe_validation_status(
                "user1",
                "bootstrap",
                "missing",
                "validated",
                None,
            )
            .await;
        assert!(matches!(result, Err(RecipeStoreError::NotFound(_))));
    }

    #[tokio::test]
    async fn update_recipe_validation_status_rejects_unknown_status_string() {
        let (typed, erased) = make_pair();
        let project = project_id("bootstrap");
        let r = sample_recipe("r1", ValidationStatus::Pending);
        save_recipe_doc(&typed, project, &r).await;
        let store = StoreBackedRecipeStore::open(erased);
        let result = store
            .update_recipe_validation_status("user1", "bootstrap", "r1", "bogus", None)
            .await;
        assert!(matches!(result, Err(RecipeStoreError::Invalid(_))));
    }

    #[tokio::test]
    async fn update_skill_validation_status_moves_through_states() {
        let (typed, erased) = make_pair();
        let project = project_id("bootstrap");
        let s = sample_skill("s1", ValidationStatus::AutoPassed);
        save_skill_doc(&typed, project, &s).await;
        let store = StoreBackedRecipeStore::open(erased);

        // auto_passed → validated (happy path)
        let resp = store
            .update_skill_validation_status("user1", "bootstrap", "s1", "validated", None)
            .await
            .expect("validate");
        assert_eq!(resp.previous_status, "auto_passed");
        assert_eq!(resp.new_status, "validated");
        assert_eq!(resp.review_attempts, 0);

        let current = store
            .get_tool_skill("user1", "bootstrap", "s1")
            .await
            .expect("get")
            .expect("some");
        assert_eq!(current.tool_skill["validation_status"], "validated");
    }

    #[tokio::test]
    async fn update_skill_review_requested_does_not_bump_review_attempts() {
        let (typed, erased) = make_pair();
        let project = project_id("bootstrap");
        let s = sample_skill("s1", ValidationStatus::AutoPassed);
        save_skill_doc(&typed, project, &s).await;
        let store = StoreBackedRecipeStore::open(erased);

        let resp = store
            .update_skill_validation_status(
                "user1",
                "bootstrap",
                "s1",
                "review_requested",
                Some("please add error handling"),
            )
            .await
            .expect("review");
        assert_eq!(resp.previous_status, "auto_passed");
        assert_eq!(resp.review_attempts, 0, "user action must not bump review_attempts");

        let current = store
            .get_tool_skill("user1", "bootstrap", "s1")
            .await
            .expect("get")
            .expect("some");
        assert_eq!(current.tool_skill["review_attempts"], 0);
        assert_eq!(current.tool_skill["review_feedback"], "please add error handling");
    }

    #[tokio::test]
    async fn shared_recipe_is_visible_but_not_writable_by_non_owner() {
        let (typed, erased) = make_pair();
        let project = project_id("bootstrap");
        let mut shared_recipe = sample_recipe("shared-r1", ValidationStatus::AutoPassed);
        shared_recipe.user_id = "__shared__".to_string();
        save_recipe_doc(&typed, project, &shared_recipe).await;
        let store = StoreBackedRecipeStore::open(erased);

        // Read path: shared recipe is visible via list_recipes because
        // load_filtered_docs uses list_memory_docs_with_shared.
        let recipes = store
            .list_recipes("user1", "bootstrap")
            .await
            .expect("list");
        assert!(
            recipes.iter().any(|r| r.id == "shared-r1"),
            "shared recipe must be visible in read listing"
        );

        // Write path: update_recipe_validation_status uses find_own_doc
        // (list_memory_docs, not with_shared), so the shared recipe is
        // invisible and the call returns NotFound — not an access error
        // that leaks the existence of the doc.
        let result = store
            .update_recipe_validation_status(
                "user1",
                "bootstrap",
                "shared-r1",
                "validated",
                None,
            )
            .await;
        assert!(
            matches!(result, Err(RecipeStoreError::NotFound(_))),
            "non-owner must not be able to mutate a shared recipe"
        );
    }

    #[tokio::test]
    async fn record_recipe_outcome_updates_wilson_and_tier() {
        let (typed, erased) = make_pair();
        let project = project_id("bootstrap");
        let r = sample_recipe("r1", ValidationStatus::Validated);
        save_recipe_doc(&typed, project, &r).await;
        let store = StoreBackedRecipeStore::open(erased);
        for _ in 0..20 {
            let resp = store
                .record_outcome(
                    "user1",
                    "bootstrap",
                    RecordOutcomeRequest {
                        id: "r1".to_string(),
                        kind: OutcomeKind::Recipe,
                        success: true,
                    },
                )
                .await
                .expect("outcome");
            assert!(resp.recorded);
        }
        let current = store
            .get_recipe("user1", "bootstrap", "r1")
            .await
            .expect("get")
            .expect("some");
        assert_eq!(current.recipe["usage_count"], 20);
        assert_eq!(current.recipe["success_count"], 20);
        assert_eq!(current.recipe["failure_count"], 0);
        assert_eq!(current.recipe["tier"], "mature");
    }

    #[tokio::test]
    async fn record_recipe_outcome_unknown_id_returns_unavailable() {
        let (_typed, erased) = make_pair();
        let store = StoreBackedRecipeStore::open(erased);
        let result = store
            .record_outcome(
                "user1",
                "bootstrap",
                RecordOutcomeRequest {
                    id: "missing".to_string(),
                    kind: OutcomeKind::Recipe,
                    success: true,
                },
            )
            .await;
        assert!(matches!(result, Err(RecipeStoreError::Unavailable(_))));
    }

    #[tokio::test]
    async fn parse_project_id_rejects_garbage() {
        assert!(parse_project_id("").is_err());
        assert!(parse_project_id("has space").is_err());
        let too_long = "x".repeat(65);
        assert!(parse_project_id(&too_long).is_err());
        assert!(parse_project_id("good-id_42").is_ok());
    }
}
