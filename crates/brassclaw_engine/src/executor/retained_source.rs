//! Contained source preflight for the currently supported retained-step adapter.
//! It cannot establish semantic Q1, behavioral validation, Q2 or activation.
//! Independent dispatches and unsupported dependent-chain binding layouts must
//! fail before execution, rather than partly running and discovering a mismatch.

use std::{collections::BTreeMap, path::Path, time::Duration};

use brassclaw_monty_host::{
    process::ProcessLimits,
    source_structure::SourceStructure,
    utility::{UtilityError, UtilityOutput, UtilityRequest, execute},
};
use brassclaw_skills::association_contract::ComponentRevisionRef;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{retained_recipe::RetainedProgram, scripting::utility_bounds};

#[derive(thiserror::Error)]
pub enum RetainedSourceError {
    #[error("retained source preflight for {component}: {reason}")]
    Invalid {
        component: Uuid,
        reason: &'static str,
    },
    #[error("retained source inspection failed for {component}")]
    Inspection {
        component: Uuid,
        #[source]
        error: Box<UtilityError>,
    },
    #[error("retained source worker returned an unexpected result for {component}")]
    Reply {
        component: Uuid,
        output: Box<UtilityOutput>,
    },
}
impl std::fmt::Debug for RetainedSourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_string())
    }
}

/// Exact immutable component reference paired with actual parser observations.
/// Private construction prevents substitution of a caller-supplied report.
pub struct InspectedSource {
    component: ComponentRevisionRef,
    observations: SourceStructure,
}
impl InspectedSource {
    pub fn component(&self) -> ComponentRevisionRef {
        self.component
    }
    pub fn observations(&self) -> &SourceStructure {
        &self.observations
    }
}
pub type InspectedSources = BTreeMap<Uuid, InspectedSource>;

/// The executor accepts only the exact program inspected by this constructor.
/// Successful construction grants no permission and activates no component.
/// A trusted catalogue owner may share this immutable value across task hosts;
/// a new component revision needs its own inspection and required reviews.
pub struct InspectedRetainedProgram {
    program: RetainedProgram,
    sources: InspectedSources,
}
impl InspectedRetainedProgram {
    pub async fn inspect(
        program: RetainedProgram,
        worker: &Path,
    ) -> Result<Self, RetainedSourceError> {
        let mut sources: InspectedSources = BTreeMap::new();
        for step in &program.program().steplist {
            let component = *program.inputs().components().get(&step.step_id).ok_or(
                RetainedSourceError::Invalid {
                    component: program.inputs().instruction().recipe().uuid,
                    reason: "selected executable reference missing",
                },
            )?;
            let invalid = |reason| RetainedSourceError::Invalid {
                component: component.uuid,
                reason,
            };
            if step.executable_code.len() > utility_bounds().max_source_bytes {
                return Err(invalid("source exceeds technical inspection capacity"));
            }
            if let Some(previous) = sources.get(&component.uuid) {
                if previous.component != component
                    || previous.observations.source_checksum
                        != format!("{:x}", Sha256::digest(step.executable_code.as_bytes()))
                {
                    return Err(invalid("same selected component has inconsistent source"));
                }
            } else {
                let output = execute(
                    worker,
                    UtilityRequest::InspectSource {
                        source: step.executable_code.clone(),
                        bounds: utility_bounds(),
                    },
                    ProcessLimits {
                        hard_memory_bytes: 64 * 1024 * 1024,
                        max_frame_bytes: 16 * 1024 * 1024,
                        response_timeout: Duration::from_secs(5),
                    },
                )
                .await
                .map_err(|error| RetainedSourceError::Inspection {
                    component: component.uuid,
                    error: Box::new(error),
                })?;
                let UtilityOutput::Inspected { structure } = output else {
                    return Err(RetainedSourceError::Reply {
                        component: component.uuid,
                        output: Box::new(output),
                    });
                };
                sources.insert(
                    component.uuid,
                    InspectedSource {
                        component,
                        observations: structure,
                    },
                );
            }
            // Check every use: repeated code may have different binding context.
            let observations = &sources[&component.uuid].observations;
            if observations.relative_imports
                || observations
                    .imports
                    .iter()
                    .any(|name| !supported_import(name))
                || !observations.reserved_name_references.is_empty()
            {
                return Err(invalid(
                    "forbidden import or executable intrinsic reference",
                ));
            }
            if observations.host_value_references != 0 {
                return Err(invalid(
                    "dynamic host access is not supported by this adapter",
                ));
            }
            if observations.result_store_sites == 0 {
                return Err(invalid("PythonCode must assign result"));
            }
            match program.binding(&step.step_id) {
                None if observations.direct_host_calls.is_empty() => {}
                Some(binding)
                    if observations.direct_host_calls.len() == 1
                        && binding.association().callable().strip_prefix("host.")
                            == Some(observations.direct_host_calls[0].attribute.as_str()) => {}
                _ => {
                    return Err(invalid(
                        "host call sites do not match the supported single-Tool binding",
                    ));
                }
            }
        }
        Ok(Self { program, sources })
    }

    pub fn source_checks(&self) -> &InspectedSources {
        &self.sources
    }
    pub fn program(&self) -> &RetainedProgram {
        &self.program
    }
}

// Match the vendored compiler's actual production stdlib surface. This is
// adapter compatibility, not a license to run module I/O: any OS request still
// needs the executor's explicit kernel-controlled boundary. os is forbidden by
// the PythonCode contract even though the interpreter can parse its import.
fn supported_import(name: &str) -> bool {
    matches!(
        name,
        "asyncio"
            | "base64"
            | "binascii"
            | "collections"
            | "copy"
            | "dataclasses"
            | "datetime"
            | "functools"
            | "itertools"
            | "json"
            | "math"
            | "pathlib"
            | "random"
            | "re"
            | "sys"
            | "time"
            | "typing"
            | "unicodedata"
    )
}
