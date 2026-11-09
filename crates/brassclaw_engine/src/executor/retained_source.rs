//! Contained source preflight for the currently supported retained-step adapter.
//! It cannot establish semantic Q1, behavioral validation, Q2 or activation.
//! Independent dispatches and unsupported dependent-chain binding layouts must
//! fail before execution, rather than partly running and discovering a mismatch.

use std::{collections::BTreeMap, path::Path, sync::Arc};

use brassclaw_monty_host::{
    source_structure::SourceStructure,
    utility::{UtilityError, UtilityLimits, UtilityOutput, UtilityRequest, execute},
};
use brassclaw_skills::{
    association_contract::ComponentRevisionRef,
    component_revision::{REVISION_LIMITS, RetainedComponentSnapshot},
    value_contract::{ContractLimits, InputContract, ValueContract},
};
use uuid::Uuid;

use super::{retained_profile::supported_import, retained_recipe::RetainedProgram};

#[derive(thiserror::Error)]
pub enum RetainedSourceError {
    #[error("retained source preflight for {component}: {reason}")]
    Invalid {
        component: Uuid,
        reason: &'static str,
    },
    #[error("retained source inspection failed for {component}: {error}")]
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

/// Actual parser observations for one closed, effect-free preload graph.
/// Construction checks every declared root export and every dependency source,
/// including host calls outside the selected invocation. This is structural
/// evidence only: it establishes neither behavior, semantic review, approval,
/// catalogue activation nor permission to execute a task.
pub struct InspectedPureComponent<'a> {
    component: ComponentRevisionRef,
    components: BTreeMap<Uuid, ComponentRevisionRef>,
    sources: BTreeMap<Uuid, &'a InspectedSource>,
    export_invocations: BTreeMap<String, String>,
}
impl<'a> InspectedPureComponent<'a> {
    pub fn component(&self) -> ComponentRevisionRef {
        self.component
    }
    pub fn components(&self) -> &BTreeMap<Uuid, ComponentRevisionRef> {
        &self.components
    }
    pub fn sources(&self) -> &BTreeMap<Uuid, &'a InspectedSource> {
        &self.sources
    }
    /// Generated invocations are derived from retained declarations, never
    /// runtime input or prose. Behavioral evidence must cover each export.
    pub fn export_invocations(&self) -> &BTreeMap<String, String> {
        &self.export_invocations
    }
}

/// The executor accepts only the exact program inspected by this constructor.
/// Successful construction grants no permission and activates no component.
/// A trusted catalogue owner may share this immutable value across task hosts;
/// a new component revision needs its own inspection and required reviews.
pub struct InspectedRetainedProgram {
    preparation_limits: UtilityLimits,
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
        let limits = UtilityLimits::from_execution(Default::default())
            .expect("default preparation policy is supported");
        Self::inspect_with_limits(program, worker, limits).await
    }

    pub fn preparation_limits(&self) -> UtilityLimits {
        self.preparation_limits
    }

    /// Product callers must pass the acknowledged instance preparation policy.
    pub async fn inspect_with_limits(
        program: RetainedProgram,
        worker: &Path,
        preparation_limits: UtilityLimits,
    ) -> Result<Self, RetainedSourceError> {
        let graph = program.inputs().instruction().snapshot().revisions();
        let fail = |component, reason| RetainedSourceError::Invalid { component, reason };
        let (libraries, sources, preload_order) = inspect_graph(
            program.inputs().instruction().snapshot(),
            program
                .inputs()
                .components()
                .values()
                .map(|reference| reference.uuid)
                .collect(),
            program.inputs().instruction().recipe().uuid,
            worker,
            preparation_limits,
        )
        .await?;
        let mut symbols = BTreeMap::new();
        for (id, library) in &libraries {
            for symbol in library.symbols() {
                symbols.insert(symbol, *id);
            }
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
            preparation_limits,
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

    /// Extract an independently reviewable pure source closure from this exact
    /// inspected selection. No latest lookup, copied source or new interpreter
    /// is needed. A dependency outside this graph, a legacy body or even an
    /// unselected/dead host call prevents this structural pure qualification.
    pub fn pure_component(
        &self,
        component: Uuid,
    ) -> Result<InspectedPureComponent<'_>, RetainedSourceError> {
        pure_component(
            self.program.inputs().instruction().snapshot(),
            &self.sources,
            component,
        )
    }
}

// Match the vendored compiler's actual production stdlib surface. This is
// adapter compatibility, not a license to run module I/O: any OS request still
// needs the executor's explicit kernel-controlled boundary. os is forbidden by
// the PythonCode contract even though the interpreter can parse its import.

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

// Shared actual-source loader for Recipe selections and independent Pure roots.
// This performs no approval or execution and never manufactures a wrapper Recipe.
async fn inspect_graph(
    snapshot: &RetainedComponentSnapshot,
    roots: Vec<Uuid>,
    owner: Uuid,
    worker: &Path,
    preparation_limits: UtilityLimits,
) -> Result<
    (
        BTreeMap<Uuid, super::retained_preload::PreloadDeclaration>,
        InspectedSources,
        Vec<Uuid>,
    ),
    RetainedSourceError,
> {
    use super::retained_preload::PreloadDeclaration;
    let graph = snapshot.revisions();
    let fail = |component, reason| RetainedSourceError::Invalid { component, reason };
    let mut wanted = std::collections::BTreeSet::new();
    let mut pending = roots;
    let mut libraries = BTreeMap::new();
    while let Some(id) = pending.pop() {
        if !wanted.insert(id) {
            continue;
        }
        let revision = graph
            .get(&id)
            .filter(|r| r.reference().class_code == 22)
            .ok_or_else(|| fail(id, "preload dependency must be retained PythonCode"))?;
        if let Some(library) =
            PreloadDeclaration::parse(revision.draft().document(), revision.draft().dependencies())
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
            owner,
            "legacy step bodies cannot share a preloaded namespace",
        ));
    }
    let preload_order = super::retained_preload::dependency_order(&libraries)
        .map_err(|reason| fail(owner, reason))?;
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
                bounds: preparation_limits.bounds,
            },
            preparation_limits.process,
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
            if observed
                .references
                .iter()
                .any(|name| !available.contains(name) && !super::retained_preload::builtin(name))
            {
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
    Ok((libraries, sources, preload_order))
}

fn pure_component<'a>(
    snapshot: &RetainedComponentSnapshot,
    observations: &'a InspectedSources,
    component: Uuid,
) -> Result<InspectedPureComponent<'a>, RetainedSourceError> {
    use super::retained_preload::PreloadDeclaration;

    let fail = |reason| RetainedSourceError::Invalid { component, reason };
    let graph = snapshot.revisions();
    let root = graph
        .get(&component)
        .filter(|revision| revision.reference().class_code == 22)
        .ok_or_else(|| fail("pure root must be retained PythonCode"))?;
    let document = root.draft().document();
    // Schema declarations are immutable revision metadata, not executing
    // task values. A larger valid task-depth setting must not violate the
    // schema parser's separate storage profile; invocation still binds and
    // checks actual values under its acknowledged per-step limits.
    InputContract::from_value(&document["input_contract"], REVISION_LIMITS)
        .map_err(|_| fail("pure root input contract is invalid"))?;
    ValueContract::result_from_value(&document["result_contract"], REVISION_LIMITS)
        .map_err(|_| fail("pure root result contract is invalid"))?;
    let library = PreloadDeclaration::parse(document, root.draft().dependencies())
        .map_err(fail)?
        .filter(|library| !library.exports.is_empty())
        .ok_or_else(|| fail("pure root requires declared preload exports"))?;
    let export_invocations = library
        .exports
        .keys()
        .map(|name| {
            library
                .invocation(name, &document["input_contract"])
                .map(|invocation| (name.clone(), invocation))
                .map_err(fail)
        })
        .collect::<Result<_, _>>()?;
    let mut components = BTreeMap::new();
    let mut sources = BTreeMap::new();
    let mut pending = vec![component];
    while let Some(id) = pending.pop() {
        if components.contains_key(&id) {
            continue;
        }
        let dependency_error = |reason| RetainedSourceError::Invalid {
            component: id,
            reason,
        };
        let revision = graph
            .get(&id)
            .filter(|revision| revision.reference().class_code == 22)
            .ok_or_else(|| dependency_error("pure dependency must be retained PythonCode"))?;
        let observed = observations
            .get(&id)
            .filter(|source| source.component() == revision.reference())
            .ok_or_else(|| dependency_error("pure dependency lacks exact source observations"))?;
        if observed.observations().preload.is_none()
            || !observed.observations().direct_host_calls.is_empty()
        {
            return Err(dependency_error(
                "pure closure contains a legacy body or host call",
            ));
        }
        pending.extend(revision.draft().dependencies());
        components.insert(id, revision.reference());
        sources.insert(id, observed);
    }
    let mut owners = BTreeMap::new();
    for (id, source) in &sources {
        let preload = source
            .observations()
            .preload
            .as_ref()
            .ok_or_else(|| fail("pure dependency lacks preload observations"))?;
        for symbol in preload.functions.keys() {
            if owners.insert(symbol.clone(), *id).is_some() {
                return Err(fail("pure dependency symbol conflict"));
            }
        }
    }
    for export in library.exports.values() {
        let calls = export_calls(&export.symbol, &owners, observations).map_err(fail)?;
        if !calls.is_empty() {
            return Err(fail("pure export resolves to a host call"));
        }
    }
    Ok(InspectedPureComponent {
        component: root.reference(),
        components,
        sources,
        export_invocations,
    })
}

/// Exact, structurally inspected original closure, independent of any consuming
/// Recipe. All exports and all source bytes are checked up front. This is not
/// behavioral evidence or approval. The review owner must bind its checked
/// original and immutable case before exposing a native invocation service.
pub struct InspectedPureSelection {
    snapshot: Arc<RetainedComponentSnapshot>,
    root: Uuid,
    sources: InspectedSources,
    preload_order: Vec<Uuid>,
}
impl InspectedPureSelection {
    pub async fn inspect(
        snapshot: Arc<RetainedComponentSnapshot>,
        root: Uuid,
        worker: &Path,
        limits: UtilityLimits,
    ) -> Result<Self, RetainedSourceError> {
        let (_, sources, preload_order) =
            inspect_graph(&snapshot, vec![root], root, worker, limits).await?;
        pure_component(&snapshot, &sources, root)?;
        Ok(Self {
            snapshot,
            root,
            sources,
            preload_order,
        })
    }

    pub fn inspected(&self) -> Result<InspectedPureComponent<'_>, RetainedSourceError> {
        pure_component(&self.snapshot, &self.sources, self.root)
    }

    /// Seal one declared export and typed input. No source or symbol is taken
    /// from case input. Missing inputs alone receive declared defaults.
    pub fn invocation(
        &self,
        export: &str,
        supplied: &serde_json::Value,
        limits: ContractLimits,
    ) -> Result<SealedPureInvocation, RetainedSourceError> {
        let fail = |reason| RetainedSourceError::Invalid {
            component: self.root,
            reason,
        };
        let inspected = self.inspected()?;
        let source = inspected
            .export_invocations()
            .get(export)
            .ok_or_else(|| fail("unknown Pure export"))?
            .clone();
        let document = self.snapshot.revisions()[&self.root].draft().document();
        let inputs = InputContract::from_value(&document["input_contract"], REVISION_LIMITS)
            .map_err(|_| fail("invalid Pure input contract"))?
            .bind_with_limits(supplied, limits)
            .map_err(|_| fail("invalid Pure case input"))?;
        let result_contract =
            ValueContract::result_from_value(&document["result_contract"], REVISION_LIMITS)
                .map_err(|_| fail("invalid Pure result contract"))?;
        let definitions = self
            .preload_order
            .iter()
            .map(|id| {
                let source = self.snapshot.revisions()[id].draft().document()["content"]
                    .as_str()
                    .ok_or_else(|| fail("inspected Pure source missing"))?
                    .to_owned();
                Ok(selected_pure_python(source))
            })
            .collect::<Result<Vec<_>, RetainedSourceError>>()?;
        Ok(SealedPureInvocation {
            component: inspected.component(),
            export: export.to_owned(),
            definitions,
            selected: selected_pure_python(source),
            inputs: serde_json::Value::Object(inputs),
            result_contract,
            limits,
        })
    }
}

fn selected_pure_python(source: String) -> brassclaw_monty_host::process::SelectedPython {
    use sha2::{Digest, Sha256};
    brassclaw_monty_host::process::SelectedPython {
        checksum: Sha256::digest(source.as_bytes()).into(),
        source,
        aliases: Default::default(),
    }
}

/// Constructed only by inspection of an exact original closure. Its executable
/// fields are visible solely to the engine's single-invocation adapter.
pub struct SealedPureInvocation {
    pub(super) component: ComponentRevisionRef,
    pub(super) export: String,
    pub(super) definitions: Vec<brassclaw_monty_host::process::SelectedPython>,
    pub(super) selected: brassclaw_monty_host::process::SelectedPython,
    pub(super) inputs: serde_json::Value,
    pub(super) result_contract: ValueContract,
    pub(super) limits: ContractLimits,
}
