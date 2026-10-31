//! Read-only detail of the actual current coherent catalogue. Kept separate
//! from editable legacy rows; declarations do not qualify model dispatch.
use crate::installed_monty_catalogue::InstalledMontyCatalogue;
use brassclaw_product_workflow::RecipeStoreError;
use brassclaw_skills::recipe_model_input::RecipeModelInput;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

fn integrity() -> RecipeStoreError {
    RecipeStoreError::Internal("retained Recipe catalogue integrity verification failed".into())
}
impl InstalledMontyCatalogue {
    pub(crate) async fn retained_recipe_detail(
        &self,
        recipe: Uuid,
    ) -> Result<Value, RecipeStoreError> {
        let _serial = self.activation.lock().await;
        self.reconcile_committed_runtime_locked().await?;
        let view = self.active.read().await.clone();
        let mut client =
            self.pool.get().await.map_err(|_| {
                RecipeStoreError::Unavailable("catalogue database unavailable".into())
            })?;
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(|_| integrity())?;
        self.verify_source_selection(&tx, &view)
            .await
            .map_err(|_| integrity())?;
        let mut selections: BTreeMap<(u64, [u8; 32]), Value> = BTreeMap::new();
        for program in &view.programs {
            let instruction = program.program().inputs().instruction();
            let reference = instruction.recipe();
            if reference.uuid != recipe {
                continue;
            }
            let original = instruction
                .snapshot()
                .revisions()
                .get(&recipe)
                .ok_or_else(integrity)?;
            if original.reference() != reference || reference.class_code != 21 {
                return Err(integrity());
            }
            let document = original.draft().document();
            let variants = document
                .get("variants")
                .and_then(Value::as_array)
                .ok_or_else(integrity)?;
            let declared = variants
                .iter()
                .map(|value| {
                    value
                        .get("variant_key")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                        .ok_or_else(integrity)
                })
                .collect::<Result<BTreeSet<_>, _>>()?;
            if declared.len() != variants.len() {
                return Err(integrity());
            }
            let variant = instruction.variant();
            let binding = RecipeModelInput::for_variant(document, &declared, &variant.variant_key)
                .map_err(|_| integrity())?;
            if &binding != instruction.model_input() {
                return Err(integrity());
            }
            let map = document.get("model_input_bindings");
            let variant_entry_present = map
                .and_then(|value| value.get("variants"))
                .and_then(Value::as_object)
                .is_some_and(|values| values.contains_key(&variant.variant_key));
            // One complete original per exact revision, not one potentially
            // large byte copy per variant. This is a transient read projection.
            let selected = selections.entry((reference.version, reference.checksum)).or_insert_with(|| json!({
                "reference":{"uuid":reference.uuid,"class_code":reference.class_code,"version":reference.version.to_string(),"checksum":hex::encode(reference.checksum)},
                "declaration_present":map.is_some(),"component_bytes":original.draft().exact_bytes(),"variants":[]
            }));
            selected["variants"].as_array_mut().ok_or_else(integrity)?.push(json!({
                "variant_key":variant.variant_key,"step_link":variant.step_link,
                "routing_enabled":view.routing_enabled(program),"variant_entry_present":variant_entry_present,
                "model_input":binding
            }));
        }
        tx.commit().await.map_err(|_| integrity())?;
        Ok(
            json!({"generation":view.generation,"selections":selections.into_values().collect::<Vec<_>>()}),
        )
    }
}
