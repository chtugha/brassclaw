//! Complete original authoring requests projected into the existing legacy rows.
//! Extra v3 declarations stay in the immutable revision; projection grants nothing.
use crate::validation_queue::ValidationQueueStore;
use brassclaw_engine::memory::retrieval_source::ComponentScope;
use brassclaw_pg::PgPool;
use brassclaw_product_workflow::RecipeStoreError;
use brassclaw_skills::review_submission_store::{RetainedReviewSubmission, ReviewSubmissionDraft};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::sync::Arc;

fn field<T: DeserializeOwned>(document: &Value, name: &str) -> Result<T, RecipeStoreError> {
    serde_json::from_value(document.get(name).cloned().ok_or_else(|| {
        RecipeStoreError::Invalid("complete explicit queue projection required".into())
    })?)
    .map_err(|_| RecipeStoreError::Invalid("invalid queue projection field".into()))
}
// A present JSON null is data for these JSONB fields. It is never defaulted to
// SQL NULL; omitted fields fail the complete projection contract above.
fn json(document: &Value, name: &str) -> Result<Option<Value>, RecipeStoreError> {
    Ok(Some(field::<Value>(document, name)?))
}
fn failed(_: impl std::fmt::Display) -> RecipeStoreError {
    RecipeStoreError::Unavailable(
        "original queued review persistence unavailable; retain original request for readback"
            .into(),
    )
}
fn python_error(error: crate::pg_python_code_store::PgPythonCodeStoreError) -> RecipeStoreError {
    match error {
        crate::pg_python_code_store::PgPythonCodeStoreError::OriginalReview(error) => {
            crate::validation_queue::original_review_error(error)
        }
        other => failed(other),
    }
}
fn recipe_error(error: crate::pg_recipe_store::PgRecipeStoreError) -> RecipeStoreError {
    match error {
        crate::pg_recipe_store::PgRecipeStoreError::OriginalReview(error) => {
            crate::validation_queue::original_review_error(error)
        }
        other => failed(other),
    }
}
fn catalogue_error(
    error: crate::pg_extension_catalogue_store::PgExtensionCatalogueStoreError,
) -> RecipeStoreError {
    match error {
        crate::pg_extension_catalogue_store::PgExtensionCatalogueStoreError::OriginalReview(
            error,
        ) => crate::validation_queue::original_review_error(error),
        other => failed(other),
    }
}
pub(crate) async fn create(
    pool: &Arc<PgPool>,
    scope: ComponentScope,
    draft: &ReviewSubmissionDraft,
    #[cfg(feature = "skills-db")] source: Option<
        &crate::validator_inventory_review::SourceReviewPreparation,
    >,
) -> Result<RetainedReviewSubmission, RecipeStoreError> {
    let d = draft.candidate_document();
    let queue = ValidationQueueStore::new(pool.clone());
    match draft.candidate().class_code {
        22=>crate::pg_python_code_store::PgPythonCodeStore::new(pool.clone()).create_and_submit_review(
            crate::pg_python_code_store::NewPgPythonCode{
                tenant_id:scope.tenant_id,user_id:scope.user_id,agent_id:scope.agent_id,project_id:scope.project_id,
                name:field(d,"name")?,description:field(d,"description")?,content:field(d,"content")?,
                prior_knowledge_content:field(d,"prior_knowledge_content")?,override_prompt_creation:field(d,"override_prompt_creation")?,
                consumer_tags:field(d,"consumer_tags")?,intent_examples:json(d,"intent_examples")?,source:"authored".into(),
                dependency_registry:json(d,"dependency_registry")?,includes:field(d,"includes")?,content_checksum:None,
            },draft,&queue, #[cfg(feature="skills-db")] source).await.map_err(python_error),
        21=>crate::pg_recipe_store::PgRecipeStore::new(pool.clone()).create_and_submit_review(
            crate::pg_recipe_store::NewPgRecipe{
                tenant_id:scope.tenant_id,user_id:scope.user_id,agent_id:scope.agent_id,project_id:scope.project_id,
                name:field(d,"name")?,description:field(d,"description")?,trigger:json(d,"trigger")?,steps:field(d,"steps")?,
                prior_knowledge_content:field(d,"prior_knowledge_content")?,override_prompt_creation:field(d,"override_prompt_creation")?,
                consumer_tags:field(d,"consumer_tags")?,intent_examples:json(d,"intent_examples")?,source:"authored".into(),
                step_descriptions:json(d,"step_descriptions")?,variants:json(d,"variants")?,dependency_registry:json(d,"dependency_registry")?,
                validates_class_code:field(d,"validates_class_code")?,
            },draft,&queue).await.map_err(recipe_error),
        23=>crate::pg_extension_catalogue_store::PgExtensionCatalogueStore::new(pool.clone()).create_and_submit_review(
            crate::pg_extension_catalogue_store::NewPgExtensionCatalogue{
                tenant_id:scope.tenant_id,user_id:scope.user_id,agent_id:scope.agent_id,project_id:scope.project_id,
                name:field(d,"name")?,description:field(d,"description")?,version:field(d,"version")?,overview_doc:field(d,"overview_doc")?,
                task_groups:field(d,"task_groups")?,child_component_ids:field(d,"child_component_ids")?,intent_index:json(d,"intent_index")?,
                prior_knowledge_content:field(d,"prior_knowledge_content")?,override_prompt_creation:field(d,"override_prompt_creation")?,
                consumer_tags:field(d,"consumer_tags")?,intent_examples:json(d,"intent_examples")?,source:"authored".into(),dependency_registry:json(d,"dependency_registry")?,
            },draft,&queue).await.map_err(catalogue_error),
        _=>Err(RecipeStoreError::Invalid("this class has no supported legacy create projection; submit the complete immutable subject independently".into())),
    }
}
