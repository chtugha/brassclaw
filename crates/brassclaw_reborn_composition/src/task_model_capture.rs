//! Capture-only declaration from an actual immutable Recipe selection. This
//! never qualifies a workflow, approves a component or authorizes model I/O.
use std::sync::Arc;

pub(crate) use crate::task_model_capture_record::PrefixFreeCaptureRecord;
use brassclaw_engine::executor::retained_source::InspectedRetainedProgram;
use brassclaw_reborn::monty_task_host::MontyTaskHost;
use brassclaw_skills::recipe_model_input::RecipeModelInput;
use brassclaw_turns::run_profile::{
    AgentLoopDriverError, LoopModelCaptureContext, LoopTaskModelSelection,
};

/// One shared task-model capture and metadata handoff, before native bindings
/// or effects. Callers retain their original preparation/admission before this
/// await; this function neither starts a task nor qualifies a Free declaration.
pub(crate) async fn capture_task_models(
    host: &Arc<MontyTaskHost>,
    admission: &Arc<crate::pg_monty_admission::PgMontyAdmission>,
    root: Option<&InspectedRetainedProgram>,
    parent: Option<Arc<dyn LoopTaskModelSelection>>,
) -> Result<(), AgentLoopDriverError> {
    let failed = |reason: &str| AgentLoopDriverError::Failed {
        reason_kind: reason.into(),
    };
    let inherited = parent.is_some();
    let capture = root
        .map(|program| {
            PrefixFreeCapture::from_program(program, host.run_context().scope.tenant_id.as_str())
                .map_err(|_| failed("monty_task_model_capture_invalid"))
        })
        .transpose()?
        .flatten();
    let selected = host
        .capture_task_model_selection(parent, capture)
        .await
        .map_err(|_| failed("monty_task_model_capture_failed"))?;
    if inherited && selected.is_none() {
        return Err(failed("monty_child_model_selection_unknown"));
    }
    #[cfg(feature = "root-llm-provider")]
    crate::task_memory_embeddings::bind(host)
        .map_err(|_| failed("monty_task_memory_binding_failed"))?;
    admission.bind_task_models(
        selected
            .as_ref()
            .map(|selection| selection.retained_metadata())
            .transpose()
            .map_err(|_| failed("monty_task_model_capture_failed"))?,
    )
}

/// Opaque native origin; it cannot be constructed from an LLM/request flag or
/// deserialized metadata. The serialized record remains observation only.
pub(crate) struct PrefixFreeCapture {
    record: PrefixFreeCaptureRecord,
}
impl PrefixFreeCapture {
    pub(crate) fn from_program(
        program: &InspectedRetainedProgram,
        namespace: &str,
    ) -> Result<Option<Arc<dyn LoopModelCaptureContext>>, ()> {
        let instruction = program.program().inputs().instruction();
        let RecipeModelInput::PrefixFreeWorkflow { purpose } = instruction.model_input() else {
            return Ok(None);
        };
        let selection = instruction.retained_selection().map_err(|_| ())?;
        let recipe = instruction.recipe();
        let record = PrefixFreeCaptureRecord {
            format: "recipe-prefix-free-capture/1".into(),
            namespace: namespace.into(),
            recipe_uuid: recipe.uuid,
            recipe_version: recipe.version,
            recipe_checksum: hex::encode(recipe.checksum),
            variant: instruction.variant().variant_key.clone(),
            selection_checksum: hex::encode(selection.checksum()),
            purpose: *purpose,
        };
        let value = serde_json::to_value(&record).map_err(|_| ())?;
        PrefixFreeCaptureRecord::parse(&value, namespace).ok_or(())?;
        Ok(Some(Arc::new(Self { record })))
    }
    pub(crate) fn record(&self, namespace: &str) -> Result<PrefixFreeCaptureRecord, ()> {
        (self.record.namespace == namespace)
            .then(|| self.record.clone())
            .ok_or(())
    }
}
impl LoopModelCaptureContext for PrefixFreeCapture {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
