//! Joint source fixtures use the existing contained workflow executor, original
//! records and receipt observer. There is no second scheduler or candidate loop.
use super::*;

async fn program(
    pool: &Arc<PgPool>,
    storage: &dyn brassclaw_resources::PgResourceStoreSettingsSource,
    original: &RetainedToolProgram,
    class: i16,
) -> Result<Arc<RetainedToolProgram>, RebornBuildError> {
    let (_, binding) = require_inventory_binding(original)?;
    let ids = crate::source_inspection_components::usage_ids(binding.tool().uuid, true);
    let mut drafts = crate::builtin_bootstrap::validator_check_drafts()
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(invalid)?;
    drafts.extend(
        brassclaw_skills::validator_components::function_source_drafts(
            ids,
            class,
            serde_json::to_value(brassclaw_engine::executor::retained_profile::profile())
                .map_err(invalid)?,
        )
        .map_err(invalid)?,
    );
    let root = brassclaw_skills::validator_components::function_source_recipe_id(class);
    let snapshot = original.inputs().instruction().snapshot();
    let mut canonical = BTreeMap::new();
    for draft in drafts {
        if canonical.insert(draft.uuid(), draft.clone()).is_some_and(
            |previous: brassclaw_skills::component_revision::ComponentRevisionDraft| {
                previous.exact_bytes() != draft.exact_bytes()
            },
        ) {
            return Err(invalid("conflicting canonical function prerequisites"));
        }
    }
    let mut needed = std::collections::BTreeSet::new();
    let mut pending = vec![root];
    while let Some(id) = pending.pop() {
        if !needed.insert(id) {
            continue;
        }
        if let Some(selected) = snapshot.revisions().get(&id) {
            if canonical
                .get(&id)
                .is_some_and(|draft| draft.exact_bytes() != selected.draft().exact_bytes())
            {
                return Err(invalid(
                    "function workflow prerequisite differs from original selected bytes",
                ));
            }
            pending.extend(selected.draft().dependencies());
        } else {
            let draft = canonical
                .get(&id)
                .ok_or_else(|| invalid("canonical function dependency missing"))?;
            pending.extend(draft.dependencies());
        }
    }
    let mut definitions = PackagedDefinitions::from_snapshot(snapshot);
    let revisions = PgComponentRevisionStore::new(pool.clone());
    for id in needed {
        if !snapshot.revisions().contains_key(&id) {
            let draft = canonical
                .get(&id)
                .ok_or_else(|| invalid("canonical function dependency missing"))?;
            io::bounded_for(
                storage,
                "function_workflow_component",
                draft.uuid(),
                definitions.retain(&revisions, draft),
            )
            .await?;
        }
    }
    let graph = io::bounded_for(
        storage,
        "function_workflow_snapshot",
        root,
        definitions.snapshot(pool.clone(), root),
    )
    .await?;
    let prepared = prepare_retained_tool_program(
        compile_retained_recipe(graph, root, "selected", WorkflowClass::Deterministic)
            .map_err(invalid)?,
    )
    .map_err(invalid)?
    .retain_approvals_from(original)
    .map_err(invalid)?;
    let (_, binding) = require_inventory_binding(&prepared)?;
    require_packaged_inventory_profile(
        &prepared,
        binding,
        InventoryReviewProfile::Function(class),
    )?;
    RetainedValidatorCoverage::from_inputs(prepared.inputs()).map_err(invalid)?;
    Ok(Arc::new(prepared))
}

pub(crate) async fn qualify_function_workflows(
    observations: &InstallationSourceObservations,
    pure: &CheckedPureCheckerClosure,
    function: &CheckedFunctionCheckerClosure,
    executor: Arc<crate::global_monty_startup::ControlledExecutorConfiguration>,
    preparation: brassclaw_monty_host::utility::UtilityLimits,
) -> Result<BTreeMap<i16, CheckedSourceWorkflow>, RebornBuildError> {
    let mut workflows = BTreeMap::new();
    if let InstallationSourceObservations::Shipped {
        pool,
        storage,
        program: original,
        parser,
    } = observations
    {
        let mut selected_prerequisites = original.clone();
        for class in [1, 2, 3, 22] {
            let program = program(pool, storage.as_ref(), &selected_prerequisites, class).await?;
            function.require_program(&program, pure)?;
            selected_prerequisites = program.clone();
            let transfer = crate::shipped_validation::Transfer::new(json!({
                "function_source_workflow":program.inputs().instruction().retained_selection().map_err(invalid)?.exact_bytes(),
                "worker_checksum":hex::encode(parser.worker.checksum()),"primary_class":class,
            }));
            transfer.install(pool).await?;
            workflows.insert(
                class,
                CheckedSourceWorkflow {
                    kind: WorkflowKind::Function(class),
                    cases: Vec::new(),
                    report: String::new(),
                    deployment: Some(ShippedWorkflow {
                        storage: storage.clone(),
                        program,
                        parser: parser.clone(),
                        transfer,
                    }),
                },
            );
        }
        return Ok(workflows);
    }
    observations.require_positive_for_qualification()?;
    let InstallationSourceObservations::ObservedPositive(profiles) = observations else {
        return Err(invalid(
            "actual joint source qualification prerequisites required",
        ));
    };
    let base = profiles
        .iter()
        .find(|profile| profile.context.selection.profile() == store::InstallationProfile::Pure)
        .ok_or_else(|| invalid("original installation inventory context missing"))?
        .context
        .clone();
    base.parser.proof.verify(&base.pool).await?;
    let identity = base.subject.candidate().uuid;
    let mut client = io::bounded_for(
        base.storage.as_ref(),
        "function_workflow_prerequisites_acquire",
        identity,
        io::acquire(&base.pool),
    )
    .await?;
    let tx = io::bounded_for(
        base.storage.as_ref(),
        "function_workflow_prerequisites_begin",
        identity,
        async {
            client
                .build_transaction()
                .isolation_level(IsolationLevel::RepeatableRead)
                .read_only(true)
                .start()
                .await
                .map_err(invalid)
        },
    )
    .await?;
    // Each complete original/proof bounds its own SQL. A whole qualification
    // gallery is not one resource-store exchange.
    let prerequisites = function.proof_references(&tx, pure).await?;
    base.parser.proof.verify_in_transaction(&tx).await?;
    io::bounded_for(
        base.storage.as_ref(),
        "function_workflow_prerequisites_commit",
        identity,
        async { tx.commit().await.map_err(invalid) },
    )
    .await?;
    client.recycle_after_commit();
    drop(client);
    let revisions = PgComponentRevisionStore::new(base.pool.clone());
    let mut selected_prerequisites = base.program.clone();
    for class in [1, 2, 3, 22] {
        let selected_program = program(
            &base.pool,
            base.storage.as_ref(),
            &selected_prerequisites,
            class,
        )
        .await?;
        base.parser.require_program(&selected_program)?;
        function.require_program(&selected_program, pure)?;
        selected_prerequisites = selected_program.clone();
        let mut cases = Vec::new();
        for fixture in crate::bootstrap_validator_approval::function_source_cases()?
            .into_iter()
            .filter(|fixture| fixture.primary_class == class)
        {
            let mut refs = Vec::new();
            for draft in &fixture.drafts {
                refs.push(
                    io::bounded_for(
                        base.storage.as_ref(),
                        "function_workflow_fixture_component",
                        draft.uuid(),
                        async {
                            revisions
                                .retain_packaged_draft(draft)
                                .await
                                .map_err(invalid)
                        },
                    )
                    .await?,
                );
            }
            let graph = io::bounded_for(
                base.storage.as_ref(),
                "function_workflow_fixture_graph",
                fixture.candidate,
                async {
                    revisions
                        .read_exact(
                            &[fixture.candidate, fixture.partner],
                            &refs,
                            brassclaw_skills::revision_store::SnapshotReadLimits::legacy_graph(),
                        )
                        .await
                        .map_err(invalid)
                },
            )
            .await?;
            let candidate = graph
                .revisions()
                .get(&fixture.candidate)
                .ok_or_else(|| invalid("function fixture candidate missing"))?
                .reference();
            let partner = graph
                .revisions()
                .get(&fixture.partner)
                .ok_or_else(|| invalid("function fixture partner missing"))?
                .reference();
            let mut package = base.subject.package_context().clone();
            package.factory_checksum=evidence::checksum(&json!({"owner":evidence::checksum(include_str!("bootstrap_function_workflow.rs")),
                "gallery":evidence::checksum(include_str!("bootstrap_function_source_cases.rs")),"original":package.factory_checksum,
                "checker_prerequisites":prerequisites}).to_string());
            let subject_draft = store::function_fixture_subject_record(
                &graph,
                candidate,
                partner,
                package,
                REVISION_LIMITS,
            )
            .map_err(invalid)?;
            retain_draft(&base.pool, base.storage.as_ref(), &subject_draft).await?;
            let (subject, selection_draft) = io::bounded_for(
                base.storage.as_ref(),
                "function_workflow_subject",
                subject_draft.reference().id,
                async {
                    let mut client = io::acquire(&base.pool).await?;
                    let tx = client
                        .build_transaction()
                        .isolation_level(IsolationLevel::RepeatableRead)
                        .read_only(true)
                        .start()
                        .await
                        .map_err(invalid)?;
                    let subject = store::read_subject_in_transaction(
                        &tx,
                        subject_draft.reference().id,
                        REVISION_LIMITS,
                    )
                    .await
                    .map_err(invalid)?
                    .ok_or_else(|| invalid("original installation function subject missing"))?;
                    let selection = store::selection_record(
                        &subject,
                        selected_program
                            .inputs()
                            .instruction()
                            .retained_selection()
                            .map_err(invalid)?
                            .exact_bytes(),
                        store::InstallationProfile::Function,
                        REVISION_LIMITS,
                    )
                    .map_err(invalid)?;
                    tx.commit().await.map_err(invalid)?;
                    client.recycle_after_commit();
                    Ok((Arc::new(subject), selection))
                },
            )
            .await?;
            retain_draft(&base.pool, base.storage.as_ref(), &selection_draft).await?;
            let selection = io::bounded_for(
                base.storage.as_ref(),
                "function_workflow_selection",
                selection_draft.reference().id,
                async {
                    let mut client = io::acquire(&base.pool).await?;
                    let tx = client
                        .build_transaction()
                        .isolation_level(IsolationLevel::RepeatableRead)
                        .read_only(true)
                        .start()
                        .await
                        .map_err(invalid)?;
                    let saved = store::read_selection_in_transaction(
                        &tx,
                        selection_draft.reference().id,
                        REVISION_LIMITS,
                    )
                    .await
                    .map_err(invalid)?
                    .ok_or_else(|| invalid("original installation function selection missing"))?;
                    tx.commit().await.map_err(invalid)?;
                    client.recycle_after_commit();
                    Ok(Arc::new(saved))
                },
            )
            .await?;
            let operation = Uuid::new_v5(
                &selection.reference().id,
                b"function-source-workflow-original",
            );
            let context = Arc::new(Context {
                pool: base.pool.clone(),
                storage: base.storage.clone(),
                kernel: base.kernel.clone(),
                worker: base.worker.clone(),
                parser: base.parser.clone(),
                subject,
                selection,
                program: selected_program.clone(),
                root: base.root.clone(),
                operation,
            });
            let kind = WorkflowKind::Function(class);
            let claim = json!({"format":kind.format("fixture"),"scope":kind.scope(),
                "base":{"package":context.subject.package_context(),"worker_checksum":hex::encode(context.worker.checksum()),
                    "global_root":revision_reference_value(context.root.reference()),"fixture_subject":context.subject.reference(),"original_operation_id":operation},
                "recipe":revision_reference_value(context.program.inputs().instruction().recipe()),"variant":"selected","selection":context.selection.reference(),
                "subject_closure":context.program.inputs().instruction().snapshot().revisions().values().map(|revision|revision_reference_value(revision.reference())).collect::<Vec<_>>(),
                "coverage":{"case_id":fixture.name,"rules":fixture.rules,"native_result_steps":["0:1","0:3","0:4","0:5","0:6","0:7","0:8","0:9","0:10","0:11"]},
                "profile":brassclaw_engine::executor::retained_profile::profile(),"prerequisites":prerequisites});
            let checked = CheckedCase {
                kind,
                context,
                claim,
                expected_rules: fixture.rules,
                completion: String::new(),
                report: String::new(),
            };
            io::retain_record(
                &base.pool,
                base.storage.as_ref(),
                checked.claim_id(),
                &checked.encode(checked.claim_id(), false, &checked.claim),
            )
            .await?;
            cases.push(checked);
        }
        workflows.insert(
            class,
            execute_workflow_package(
                CheckedSourceWorkflow {
                    kind: WorkflowKind::Function(class),
                    deployment: None,
                    cases,
                    report: String::new(),
                },
                executor.clone(),
                preparation,
            )
            .await?,
        );
    }
    Ok(workflows)
}
