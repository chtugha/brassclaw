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
    preload_order: Vec<Uuid>,
    invocations: BTreeMap<String, String>,
}
impl InspectedRetainedProgram {
    pub async fn inspect(
        program: RetainedProgram,
        worker: &Path,
    ) -> Result<Self, RetainedSourceError> {
        use super::retained_preload::PreloadDeclaration;
        let graph = program.inputs().instruction().snapshot().revisions();
        let fail = |component, reason| RetainedSourceError::Invalid { component, reason };
        let mut wanted = std::collections::BTreeSet::new();
        let mut pending: Vec<_> = program
            .inputs()
            .components()
            .values()
            .map(|reference| reference.uuid)
            .collect();
        let mut libraries = BTreeMap::new();
        while let Some(id) = pending.pop() {
            if !wanted.insert(id) {
                continue;
            }
            let revision = graph
                .get(&id)
                .filter(|r| r.reference().class_code == 22)
                .ok_or_else(|| fail(id, "preload dependency must be retained PythonCode"))?;
            if let Some(library) = PreloadDeclaration::parse(
                revision.draft().document(),
                revision.draft().dependencies(),
            )
            .map_err(|reason| fail(id, reason))?
            {
                pending.extend(library.dependencies.iter().copied());
                libraries.insert(id, library);
            } else if !revision.draft().dependencies().is_empty() {
                return Err(fail(id, "dependency requires a qualified preload library"));
            }
        }
        if !libraries.is_empty() && libraries.len() != wanted.len() {
            return Err(fail(
                program.inputs().instruction().recipe().uuid,
                "legacy step bodies cannot share a preloaded namespace",
            ));
        }
        let preload_order = super::retained_preload::dependency_order(&libraries)
            .map_err(|reason| fail(program.inputs().instruction().recipe().uuid, reason))?;
        let mut symbols = BTreeMap::new();
        let mut public_names = BTreeMap::new();
        for (id, library) in &libraries {
            for symbol in library.symbols() {
                if symbols.insert(symbol, *id).is_some() {
                    return Err(fail(*id, "preload symbol collision"));
                }
            }
            for name in library.exports.keys() {
                if public_names.insert(name, *id).is_some() {
                    return Err(fail(*id, "public export collision"));
                }
            }
        }
        let mut sources: InspectedSources = BTreeMap::new();
        for id in &wanted {
            let revision = &graph[id];
            let source = revision.draft().document()["content"]
                .as_str()
                .ok_or_else(|| fail(*id, "retained source missing"))?;
            let output = execute(
                worker,
                UtilityRequest::InspectSource {
                    source: source.to_owned(),
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
                component: *id,
                error: Box::new(error),
            })?;
            let UtilityOutput::Inspected { structure } = output else {
                return Err(RetainedSourceError::Reply {
                    component: *id,
                    output: Box::new(output),
                });
            };
            if structure.relative_imports
                || structure.imports.iter().any(|name| !supported_import(name))
                || !structure.reserved_name_references.is_empty()
                || structure.host_value_references != 0
            {
                return Err(fail(
                    *id,
                    "forbidden import, intrinsic or dynamic host access",
                ));
            }
            if let Some(library) = libraries.get(id) {
                let observed = structure
                    .preload
                    .as_ref()
                    .ok_or_else(|| fail(*id, "effectful preload definition"))?;
                let functions: BTreeMap<_, _> = library
                    .exports
                    .values()
                    .map(|entry| (entry.symbol.clone(), entry.parameters.clone()))
                    .chain(
                        library
                            .private_functions
                            .iter()
                            .map(|(name, args)| (name.clone(), args.clone())),
                    )
                    .collect();
                let mut available = library.symbols();
                let mut dependencies = library.dependencies.clone();
                let mut visited = std::collections::BTreeSet::new();
                while let Some(dependency) = dependencies.pop() {
                    if visited.insert(dependency) {
                        let declared = libraries
                            .get(&dependency)
                            .ok_or_else(|| fail(*id, "undeclared library dependency"))?;
                        available.extend(declared.symbols());
                        dependencies.extend(&declared.dependencies);
                    }
                }
                if observed.references.iter().any(|name| {
                    !available.contains(name) && !super::retained_preload::builtin(name)
                }) {
                    return Err(fail(*id, "undeclared preload symbol reference"));
                }
                if observed.functions != functions
                    || observed.constants != library.constants.iter().cloned().collect()
                    || structure.imports != library.imports.iter().cloned().collect()
                {
                    return Err(fail(*id, "preload declarations differ from actual source"));
                }
            } else if structure.result_store_sites == 0 {
                return Err(fail(*id, "legacy PythonCode must assign result"));
            }
            sources.insert(
                *id,
                InspectedSource {
                    component: revision.reference(),
                    observations: structure,
                },
            );
        }
        let recipe = graph[&program.inputs().instruction().recipe().uuid]
            .draft()
            .document();
        let recipe_id = program.inputs().instruction().recipe().uuid;
        let selected_invocations = match recipe.get("invocations") {
            None => None,
            Some(layouts) => {
                let layouts = layouts
                    .as_object()
                    .ok_or_else(|| fail(recipe_id, "invalid invocation layouts"))?;
                let selected = layouts.get(&program.inputs().instruction().variant().variant_key);
                if selected.is_some_and(|layout| {
                    layout
                        .as_object()
                        .is_none_or(|entries| entries.values().any(|value| !value.is_string()))
                }) {
                    return Err(fail(recipe_id, "invalid invocation export selection"));
                }
                selected
            }
        };
        let mut invocations = BTreeMap::new();
        for step in &program.program().steplist {
            let component = program.inputs().components()[&step.step_id];
            let document = graph[&component.uuid].draft().document();
            let export = if let Some(library) = libraries.get(&component.uuid) {
                Some(
                    selected_invocations
                        .and_then(|v| v.get(&step.step_id))
                        .and_then(serde_json::Value::as_str)
                        .or(library.default_export.as_deref())
                        .ok_or_else(|| {
                            fail(component.uuid, "explicit invocation export required")
                        })?,
                )
            } else {
                None
            };
            let calls = if let Some(export) = export {
                let symbol = &libraries[&component.uuid]
                    .exports
                    .get(export)
                    .ok_or_else(|| fail(component.uuid, "unknown invocation export"))?
                    .symbol;
                export_calls(symbol, &symbols, &sources)
                    .map_err(|reason| fail(component.uuid, reason))?
            } else {
                sources[&component.uuid]
                    .observations
                    .direct_host_calls
                    .iter()
                    .flat_map(|call| {
                        std::iter::repeat_n(
                            call.attribute.clone(),
                            if call.repeatable { 2 } else { 1 },
                        )
                    })
                    .take(2)
                    .collect()
            };
            match program.binding(&step.step_id) {
                None if calls.is_empty() => {}
                Some(binding)
                    if calls.len() == 1
                        && binding.association().callable().strip_prefix("host.")
                            == Some(calls[0].as_str()) => {}
                _ => {
                    return Err(fail(
                        component.uuid,
                        "transitive calls differ from the selected single-Tool usage",
                    ));
                }
            }
            if let Some(library) = libraries.get(&component.uuid) {
                let export = export.expect("selected library export checked above");
                let invocation = library
                    .invocation(export, &document["input_contract"])
                    .map_err(|reason| fail(component.uuid, reason))?;
                if let Some(binding) = program.binding(&step.step_id) {
                    // Every public entry of this Skill must implement the same
                    // one-Tool usage; unselected helpers stay private.
                    for (name, entry) in &library.exports {
                        // An unselected public entry is still part of this
                        // exact Skill interface. Qualify its typed invocation
                        // now, before this library can have any effects.
                        library
                            .invocation(name, &document["input_contract"])
                            .map_err(|reason| fail(component.uuid, reason))?;
                        let calls = export_calls(&entry.symbol, &symbols, &sources)
                            .map_err(|reason| fail(component.uuid, reason))?;
                        if calls.len() != 1
                            || binding.association().callable().strip_prefix("host.")
                                != Some(calls[0].as_str())
                        {
                            return Err(fail(
                                component.uuid,
                                "public exports must implement one compatible Tool usage",
                            ));
                        }
                    }
                    let skill = graph[&binding.skill().uuid].draft().document();
                    let mut private_symbols: Vec<_> = library
                        .private_functions
                        .keys()
                        .cloned()
                        .chain(library.constants.iter().cloned())
                        .collect();
                    private_symbols.sort();
                    let association: serde_json::Value = serde_json::from_str(
                        binding.association().exact_bytes(),
                    )
                    .map_err(|_| fail(component.uuid, "invalid retained Skill association"))?;
                    if skill["interface"]
                        != serde_json::json!({"format":"skill-interface/1","python_code_uuid":component.uuid,
                        "exports":document["preload"]["exports"],"inputs":document["input_contract"],"result":document["result_contract"],
                        "failure":association["failure"],"dependencies":library.dependencies,"private_symbols":private_symbols})
                    {
                        return Err(fail(
                            component.uuid,
                            "Skill interface differs from retained library",
                        ));
                    }
                }
                invocations.insert(step.step_id.clone(), invocation);
            }
        }
        if let Some(layout) = selected_invocations
            && layout
                .as_object()
                .is_none_or(|v| v.keys().any(|step| !invocations.contains_key(step)))
        {
            return Err(fail(
                program.inputs().instruction().recipe().uuid,
                "unknown invocation layout step",
            ));
        }
        Ok(Self {
            program,
            sources,
            preload_order,
            invocations,
        })
    }

    pub fn preload_order(&self) -> &[Uuid] {
        &self.preload_order
    }
    pub fn invocation(&self, step: &str) -> Option<&str> {
        self.invocations.get(step).map(String::as_str)
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

// Conservative structural qualification, not behavioral approval. Tool-calling
// recursion, closures, first-class aliases and repeatable dispatches require a
// richer occurrence/binding adapter and are rejected before any effect here.
fn export_calls(
    symbol: &str,
    owners: &BTreeMap<String, Uuid>,
    sources: &InspectedSources,
) -> Result<Vec<String>, &'static str> {
    let scope = |name: &str| {
        owners.get(name).and_then(|owner| {
            sources[owner]
                .observations
                .preload
                .as_ref()
                .and_then(|library| library.scopes.get(name))
        })
    };
    let mut needed = std::collections::BTreeSet::new();
    let mut pending = vec![symbol.to_owned()];
    while let Some(name) = pending.pop() {
        if let Some(function) = scope(&name)
            && needed.insert(name)
        {
            pending.extend(
                function
                    .calls
                    .keys()
                    .chain(&function.value_references)
                    .cloned(),
            );
        }
    }
    // An iterative graph walk keeps adversarial dependency depth off the Rust
    // stack. No model input or runtime data becomes a function name here.
    let mut summaries: BTreeMap<String, Vec<String>> = BTreeMap::new();
    while !needed.is_empty() {
        let name = needed
            .iter()
            .find(|name| {
                let function = scope(name).expect("collected function scope");
                function
                    .calls
                    .keys()
                    .chain(&function.value_references)
                    .all(|dependency| {
                        scope(dependency).is_none() || summaries.contains_key(dependency)
                    })
            })
            .cloned()
            .ok_or("recursive function graph requires unsupported invocation qualification")?;
        needed.remove(&name);
        let function = scope(&name).expect("collected function scope");
        let report = &sources[&owners[&name]].observations;
        let mut calls: Vec<_> = report
            .direct_host_calls
            .iter()
            .filter(|call| function.start <= call.start && call.end <= function.end)
            .flat_map(|call| {
                std::iter::repeat_n(call.attribute.clone(), if call.repeatable { 2 } else { 1 })
            })
            .take(2)
            .collect();
        for (dependency, count) in &function.calls {
            if let Some(nested) = summaries.get(dependency) {
                for _ in 0..(*count).min(2) {
                    calls.extend(nested.iter().cloned());
                    calls.truncate(2);
                }
            }
        }
        if function
            .value_references
            .iter()
            .any(|name| summaries.get(name).is_some_and(|calls| !calls.is_empty()))
        {
            return Err(
                "first-class Tool-calling function requires unsupported binding qualification",
            );
        }
        if function.nested_functions && !calls.is_empty() {
            return Err("Tool-calling closures require unsupported invocation qualification");
        }
        summaries.insert(name, calls);
    }
    Ok(summaries.remove(symbol).unwrap_or_default())
}
