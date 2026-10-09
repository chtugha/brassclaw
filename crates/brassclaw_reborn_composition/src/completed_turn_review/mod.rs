//! Installation-owned internal review delivery into the existing global service.
pub(crate) mod backend;
mod delivery;
mod ports;
use crate::RebornBuildError;
use brassclaw_engine::{
    executor::{retained_recipe::RetainedProgram, retained_source::InspectedRetainedProgram},
    memory::{
        retained_instruction::{WorkflowClass, compile_retained_recipe},
        retained_tools::prepare_retained_tool_program,
    },
};
use brassclaw_host_runtime::NativeExecutableImage;
use brassclaw_pg::PgPool;
use brassclaw_skills::{
    completed_turn_review_components::{self as package, ReviewComponents, ReviewUsage},
    component_revision::ComponentRevisionDraft,
    revision_store::PgComponentRevisionStore,
};
pub(crate) use delivery::{ReviewOwner, ReviewSource};
use serde_json::json;
use std::{path::Path, sync::Arc};
fn invalid(reason: impl Into<String>) -> RebornBuildError {
    RebornBuildError::InvalidConfig {
        reason: reason.into(),
    }
}
pub(crate) struct Package {
    inspected: Arc<InspectedRetainedProgram>,
    image: Arc<NativeExecutableImage>,
    ids: ReviewComponents,
    selection: String,
    instructions: String,
}
impl Package {
    pub(crate) async fn boot(
        pool: Arc<PgPool>,
        worker: &Path,
    ) -> Result<Arc<Self>, RebornBuildError> {
        let image = NativeExecutableImage::capture(1024 * 1024 * 1024)
            .await
            .map_err(|e| invalid(e.to_string()))?;
        let ids = ReviewComponents {
            recipe: crate::installed_monty_catalogue::id("review:recipe"),
            prepare_request: crate::installed_monty_catalogue::id("review:formatter"),
            usages: package::OPERATIONS.map(|op| ReviewUsage {
                tool: crate::installed_monty_catalogue::id(&format!("review:{op}:tool")),
                tool_skill: crate::installed_monty_catalogue::id(&format!("review:{op}:binding")),
                skill: crate::installed_monty_catalogue::id(&format!("review:{op}:skill")),
                python_code: crate::installed_monty_catalogue::id(&format!("review:{op}:code")),
                callable: format!("host.review_{op}"),
                capability_id: format!("host.review_{op}"),
            }),
        };
        let drafts = package::drafts(&ids).map_err(|e| invalid(e.to_string()))?;
        let mut tools = Vec::new();
        for usage in &ids.usages {
            let code = drafts
                .iter()
                .find(|d| d.uuid() == usage.python_code)
                .ok_or_else(|| invalid("review code missing"))?;
            tools.push(ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1",
                "uuid":usage.tool,"class_code":0,"document":{"capability_id":usage.capability_id,
                    "callable":usage.callable,"input_contract":code.document()["input_contract"],
                    "implementation":{"format":"packaged-completed-turn-review/1","adapter":"ReviewPrimitive/1",
                        "artifact_checksum":hex::encode(image.checksum())}},"dependencies":[],"association":null}).to_string())
                .map_err(|e|invalid(e.to_string()))?);
        }
        let store = PgComponentRevisionStore::new(pool.clone());
        let mut refs = Vec::new();
        for draft in tools.into_iter().chain(drafts) {
            refs.push(
                store
                    .retain_packaged_draft(&draft)
                    .await
                    .map_err(|e| invalid(e.to_string()))?,
            );
        }
        let snapshot = Arc::new(
            store
                .read_exact(&[ids.recipe], &refs)
                .await
                .map_err(|e| invalid(e.to_string()))?,
        );
        package::verify_package(&ids, &snapshot).map_err(|e| invalid(e.to_string()))?;
        let instructions = snapshot
            .revisions()
            .get(&ids.prepare_request)
            .and_then(|r| r.draft().document()["analysis_instructions"].as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| invalid("retained review instructions missing"))?
            .to_owned();
        let program = Arc::new(
            prepare_retained_tool_program(
                compile_retained_recipe(
                    snapshot,
                    ids.recipe,
                    "selected",
                    WorkflowClass::RequiresModel,
                )
                .map_err(|e| invalid(e.to_string()))?,
            )
            .map_err(|e| invalid(e.to_string()))?,
        );
        let inspected = Arc::new(
            InspectedRetainedProgram::inspect(RetainedProgram::Tools(program), worker)
                .await
                .map_err(|e| invalid(e.to_string()))?,
        );
        let selection = inspected
            .program()
            .inputs()
            .instruction()
            .retained_selection()
            .map_err(|e| invalid(e.to_string()))?
            .exact_bytes()
            .to_owned();
        let client = pool.get().await.map_err(|e| invalid(e.to_string()))?;
        for usage in &ids.usages {
            client.execute("INSERT INTO brassclaw_instance_tool_settings(tool_id) VALUES($1) ON CONFLICT(tool_id) DO NOTHING",&[&usage.tool])
                .await.map_err(|e|invalid(e.to_string()))?;
        }
        Ok(Arc::new(Self {
            inspected,
            image,
            ids,
            selection,
            instructions,
        }))
    }
}
