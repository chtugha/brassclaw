//! Operator deployment preparation and exact proposal provenance checks.
//! No component audit, approval production, task sequencing or effect dispatch.
use std::collections::{BTreeMap, BTreeSet};

use brassclaw_product_workflow::{
    ActivateCatalogueRequest, CatalogueRecipeSelection, ComponentDeploymentPlan,
    DeploymentSubjectPin, PrepareComponentDeploymentRequest, RecipeStoreError,
};
use brassclaw_skills::review_submission_store::{
    PgReviewSubmissionStore, RetainedReviewSubmission,
};
use tokio_postgres::{IsolationLevel, Transaction};
use uuid::Uuid;

use crate::installed_monty_catalogue::InstalledMontyCatalogue;

fn invalid(reason: &str) -> RecipeStoreError {
    RecipeStoreError::Invalid(reason.into())
}
fn storage(_: impl std::fmt::Display) -> RecipeStoreError {
    RecipeStoreError::Unavailable("component deployment storage unavailable".into())
}
fn id(text: &str) -> Result<Uuid, RecipeStoreError> {
    Uuid::parse_str(text)
        .ok()
        .filter(|id| !id.is_nil() && id.to_string() == text)
        .ok_or_else(|| invalid("canonical non-nil UUID required"))
}
async fn subject(
    tx: &Transaction<'_>,
    pin: &DeploymentSubjectPin,
) -> Result<RetainedReviewSubmission, RecipeStoreError> {
    if pin.subject_checksum.len() != 64
        || !pin
            .subject_checksum
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(invalid("exact reviewed subject checksum required"));
    }
    let value = PgReviewSubmissionStore::read_in_transaction(tx, id(&pin.submission_id)?)
        .await
        .map_err(storage)?
        .ok_or_else(|| RecipeStoreError::NotFound("deployment subject not found".into()))?;
    if value.checksum != pin.subject_checksum {
        return Err(RecipeStoreError::Conflict(
            "deployment subject differs from inspected bytes".into(),
        ));
    }
    if value.candidate.class_code != 21 {
        return Err(invalid(
            "deploy components through their complete Recipe submission graph",
        ));
    }
    Ok(value)
}
async fn previous(
    tx: &Transaction<'_>,
    generation: Uuid,
) -> Result<Vec<CatalogueRecipeSelection>, RecipeStoreError> {
    let row = tx.query_one("SELECT catalogue_bytes,checksum FROM brassclaw_monty_boot_catalogues WHERE catalogue_id=$1", &[&generation]).await.map_err(storage)?;
    let bytes: String = row.get(0);
    use sha2::{Digest, Sha256};
    if hex::encode(Sha256::digest(bytes.as_bytes())) != row.get::<_, String>(1) {
        return Err(invalid("catalogue integrity failed"));
    }
    let value = crate::authored_monty_catalogue::catalogue_document(&bytes)
        .map_err(|_| invalid("catalogue integrity failed"))?;
    match value["format"].as_str() {
        Some("installation-monty-catalogue/1") => Ok(Vec::new()),
        Some(
            "authored-monty-catalogue/1"
            | "authored-monty-catalogue/2"
            | "authored-monty-catalogue/3",
        ) => serde_json::from_value(value["recipes"].clone())
            .map_err(|_| invalid("catalogue selection invalid")),
        _ => Err(invalid("unsupported catalogue format")),
    }
}

/// Called again inside the existing serialized CAS publication transaction.
/// Pins cannot supply trusted evidence; prepare_authored still resolves approvals.
pub(crate) async fn verify_publication(
    tx: &Transaction<'_>,
    current: Uuid,
    request: &ActivateCatalogueRequest,
    policy: brassclaw_host_api::MontyExecutionLimits,
) -> Result<(), RecipeStoreError> {
    if request.deployment_subjects.is_empty() {
        return Ok(());
    }
    if request.deployment_subjects.len() as u64 > u64::from(policy.max_catalogue_recipes) {
        return Err(invalid("deployment subject capacity exceeded"));
    }
    let prior = previous(tx, current).await?;
    let mut changed = BTreeSet::new();
    for pin in &request.deployment_subjects {
        let retained = subject(tx, pin).await?;
        let root = retained.candidate.uuid.to_string();
        if !changed.insert(root.clone()) {
            return Err(invalid("duplicate deployment Recipe subject"));
        }
        let recipe = request
            .recipes
            .iter()
            .find(|r| r.recipe_id == root)
            .ok_or_else(|| invalid("deployment subject has no selected Recipe"))?;
        let selected: BTreeSet<_> = recipe
            .components
            .iter()
            .map(crate::pg_review_submission::reference)
            .collect::<Result<_, _>>()?;
        let expected: BTreeSet<_> = retained
            .dependencies
            .iter()
            .copied()
            .chain(std::iter::once(retained.candidate))
            .collect();
        if selected != expected || selected.len() != recipe.components.len() {
            return Err(invalid("deployment graph differs from retained subject"));
        }
        if let Some(old) = prior.iter().find(|r| r.recipe_id == root) {
            let base = old
                .components
                .iter()
                .find(|c| c.uuid == root)
                .ok_or_else(|| invalid("active Recipe reference missing"))?;
            if retained.base != Some(crate::pg_review_submission::reference(base)?) {
                return Err(RecipeStoreError::Conflict(
                    "deployment base is not the active Recipe revision".into(),
                ));
            }
        }
    }
    if request.disabled_recipe_ids.is_some() {
        return Err(invalid("deployment cannot also change routing choices"));
    }
    // A deployment merges these subjects; it cannot withdraw or silently change
    // an unrelated active Recipe. General withdrawal stays in the catalogue API.
    for old in &prior {
        if changed.contains(&old.recipe_id) {
            continue;
        }
        let new = request
            .recipes
            .iter()
            .find(|r| r.recipe_id == old.recipe_id)
            .ok_or_else(|| invalid("deployment drops an unrelated Recipe"))?;
        if serde_json::to_value(new).map_err(storage)?
            != serde_json::to_value(old).map_err(storage)?
        {
            return Err(invalid("deployment changes an unrelated Recipe"));
        }
    }
    if request.recipes.iter().any(|r| {
        !changed.contains(&r.recipe_id) && !prior.iter().any(|old| old.recipe_id == r.recipe_id)
    }) {
        return Err(invalid("deployment adds an unpinned Recipe"));
    }
    Ok(())
}

pub(crate) async fn prepare(
    owner: &InstalledMontyCatalogue,
    request: PrepareComponentDeploymentRequest,
) -> Result<ComponentDeploymentPlan, RecipeStoreError> {
    id(&request.activation_id)?;
    let expected = id(&request.expected_generation)?;
    let policy = owner.preparation_policy().await.map_err(|_| {
        RecipeStoreError::Unavailable("acknowledged catalogue settings unavailable".into())
    })?;
    if request.recipes.is_empty()
        || request.recipes.len() as u64 > u64::from(policy.max_catalogue_recipes)
        || serde_json::to_vec(&request)
            .map_err(|_| invalid("deployment request cannot be encoded"))?
            .len() as u64
            > policy.max_catalogue_bytes
    {
        return Err(invalid("deployment exceeds catalogue preparation capacity"));
    }
    let mut client = owner.pool.get().await.map_err(storage)?;
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await
        .map_err(storage)?;
    let current: Uuid = tx
        .query_one(
            "SELECT catalogue_id FROM brassclaw_monty_active_catalogue WHERE singleton",
            &[],
        )
        .await
        .map_err(storage)?
        .get(0);
    if expected != current {
        return Err(RecipeStoreError::Conflict(
            "catalogue generation changed; prepare again".into(),
        ));
    }
    let prior = previous(&tx, current).await?;
    let mut recipes: BTreeMap<_, _> = prior
        .iter()
        .cloned()
        .map(|r| (r.recipe_id.clone(), r))
        .collect();
    let mut replaced_recipe_ids = Vec::new();
    let mut changed = BTreeSet::new();
    let mut pins = Vec::new();
    for selected in request.recipes {
        let retained = subject(&tx, &selected.subject).await?;
        let root = retained.candidate.uuid.to_string();
        if !changed.insert(root.clone()) {
            return Err(invalid("duplicate deployment Recipe subject"));
        }
        let components = retained
            .dependencies
            .iter()
            .copied()
            .chain(std::iter::once(retained.candidate))
            .map(crate::pg_review_submission::selection)
            .collect();
        if recipes
            .insert(
                root.clone(),
                CatalogueRecipeSelection {
                    recipe_id: root.clone(),
                    workflow_class: selected.workflow_class,
                    components,
                    association_approvals: selected.association_approvals,
                },
            )
            .is_some()
        {
            replaced_recipe_ids.push(root);
        }
        pins.push(selected.subject);
    }
    let preserved_recipe_ids = prior
        .iter()
        .filter(|r| !changed.contains(&r.recipe_id))
        .map(|r| r.recipe_id.clone())
        .collect();
    let activation = ActivateCatalogueRequest {
        activation_id: request.activation_id,
        expected_generation: request.expected_generation,
        recipes: recipes.into_values().collect(),
        deployment_subjects: pins,
        disabled_recipe_ids: None,
    };
    verify_publication(&tx, current, &activation, policy).await?;
    // Mechanical assembly only: reuse trusted exact-combination approvals and
    // supported retained adapters/exports. Never execute or re-audit the usage.
    owner
        .prepare_authored(&tx, &activation.recipes, policy)
        .await?;
    tx.commit().await.map_err(storage)?;
    Ok(ComponentDeploymentPlan {
        activation,
        preserved_recipe_ids,
        replaced_recipe_ids,
        catalogue_activated: false,
    })
}
