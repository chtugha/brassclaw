//! Narrow original qualification readback. No fixture is rerun here. Native
//! facts, actual VM/worker quiescence and both durable close operations must
//! already exist before completion metadata or a checked handle is produced.
use super::*;
use brassclaw_turns::{
    PgTurnStateStore, TurnRunId, TurnScope, TurnSpawnTreeStateStore, TurnStatus,
};
use tokio_postgres::Transaction;

pub(super) struct Subject {
    pub(super) program: Arc<RetainedToolProgram>,
    storage: Arc<dyn brassclaw_resources::PgResourceStoreSettingsSource>,
    pub(super) worker: Arc<PackagedExecutableImage>,
    pub(super) manifest: Value,
    pub(super) intent: Uuid,
    pub(super) behavior: Uuid,
    pub(super) quiescence: Uuid,
    pub(super) completion: Uuid,
    components: Vec<Value>,
    association: String,
}
impl Subject {
    pub(super) fn new(
        program: Arc<RetainedToolProgram>,
        worker: Arc<PackagedExecutableImage>,
        qualification_id: Uuid,
        storage: Arc<dyn brassclaw_resources::PgResourceStoreSettingsSource>,
        root: &brassclaw_skills::orchestrator_contract::GlobalRootDefinition,
    ) -> Result<Arc<Self>, RebornBuildError> {
        let (_, binding) = require_inventory_binding(&program)?;
        let components=binding.combination().iter().map(|r|json!({"uuid":r.uuid,"class_code":r.class_code,"version":r.version,"checksum":hex::encode(r.checksum)})).collect::<Vec<_>>();
        let manifest = json!({"format":"inventory-qualification-subject/1",
            "selection_bytes":program.inputs().instruction().retained_selection().map_err(invalid)?.exact_bytes(),
            "association_bytes":binding.association().exact_bytes(),"components":components,"worker_checksum":hex::encode(worker.checksum()),
            "native":program.inputs().instruction().snapshot().revisions()[&binding.tool().uuid].draft().document()["implementation"]["identity"],
            "owner_code_checksum":crate::bootstrap_evidence::checksum(include_str!("bootstrap_inventory_qualification.rs")),
            "primitive_owner_checksum":crate::bootstrap_evidence::checksum(include_str!("bootstrap_inventory_primitives.rs")),
            "completion_owner_checksum":crate::bootstrap_evidence::checksum(include_str!("bootstrap_inventory_completion.rs")),
            "executor_owner_checksum":crate::bootstrap_evidence::checksum(include_str!("global_monty_startup.rs")),
            "root_checksum":hex::encode(root.source_checksum()),"root_reference":root.reference()});
        let intent = Uuid::new_v5(&qualification_id, b"inventory-qualification-intent/1");
        let association = crate::bootstrap_evidence::checksum(binding.association().exact_bytes());
        if program.inputs().instruction().recipe().uuid
            != brassclaw_skills::global_bootstrap_components::packaged_component_id(
                "validator:inspection:inventory:qualification-recipe",
            )
        {
            return Err(invalid("qualification private workflow identity differs"));
        }
        Ok(Arc::new(Self {
            program,
            storage,
            worker,
            manifest,
            intent,
            behavior: Uuid::new_v5(&intent, b"actual-observations"),
            quiescence: Uuid::new_v5(&intent, b"actual-quiescence-observation"),
            completion: Uuid::new_v5(&intent, b"completed-original"),
            components,
            association,
        }))
    }
    pub(super) fn record(&self, id: Uuid, report: Value, complete: bool) -> String {
        evidence_record(
            id,
            "behavior",
            complete,
            &self.association,
            &self.components,
            report,
        )
    }
    /// Absence alone permits a new qualification. An existing incomplete
    /// original is an error requiring reconciliation, never a replay license.
    pub(super) async fn read(
        self: &Arc<Self>,
        pool: &Arc<PgPool>,
    ) -> Result<Option<InventoryQualification>, RebornBuildError> {
        primitives::bounded(self.storage.as_ref(), self.read_original(pool)).await
    }
    async fn read_original(
        self: &Arc<Self>,
        pool: &Arc<PgPool>,
    ) -> Result<Option<InventoryQualification>, RebornBuildError> {
        let mut client = primitives::EvidenceClient::acquire(pool).await?;
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(invalid)?;
        let Some(intent_bytes) = crate::bootstrap_evidence::read_evidence(&tx, self.intent).await?
        else {
            tx.commit().await.map_err(invalid)?;
            client.recycle_after_commit();
            return Ok(None);
        };
        let intent = strict_json(&intent_bytes, REVISION_LIMITS).map_err(invalid)?;
        exact_fields(&intent["report"], &["header"])?;
        let header = &intent["report"]["header"];
        exact_fields(
            header,
            &[
                "format",
                "qualification_id",
                "manifest",
                "recipe_selection",
                "association",
                "worker_checksum",
                "native",
                "run_id",
                "runner_id",
                "claim_checksum",
                "primitive_scope",
                "primitive_invocation",
                "sources",
                "invalid_primitive_invocation",
                "invalid_arguments",
                "denied_primitive_invocation",
                "capacity_primitive_invocation",
                "capacity_fixture_profile",
                "capacity_fixture_checksum",
                "preparation_bounds",
                "executor_configuration",
                "executor_configuration_checksum",
                "reservation_checksum",
            ],
        )?;
        let qualification_id: Uuid =
            serde_json::from_value(header["qualification_id"].clone()).map_err(invalid)?;
        let _: brassclaw_host_api::ResourceScope = exact_typed(&header["primitive_scope"])?;
        if qualification_id.is_nil()
            || Uuid::new_v5(&qualification_id, b"inventory-qualification-intent/1") != self.intent
            || header["format"] != "contained-inventory-qualification/1"
            || header["manifest"] != self.manifest
            || header["recipe_selection"] != self.manifest["selection_bytes"]
            || header["association"] != self.manifest["association_bytes"]
            || header["worker_checksum"] != self.manifest["worker_checksum"]
            || header["native"] != self.manifest["native"]
            || intent_bytes
                != evidence_record(
                    self.intent,
                    "q1",
                    false,
                    &self.association,
                    &self.components,
                    intent["report"].clone(),
                )
        {
            return Err(invalid("qualification original subject or intent differs"));
        }
        let root_reference =
            serde_json::from_value(self.manifest["root_reference"].clone()).map_err(invalid)?;
        brassclaw_skills::revision_store::PgComponentRevisionStore::read_revision_in_transaction(
            &tx,
            root_reference,
        )
        .await
        .map_err(invalid)?;
        let reservation_id = Uuid::new_v5(&self.intent, b"pre-admission-intent");
        let reservation = required(&tx, reservation_id).await?;
        let reserve = strict_json(&reservation, REVISION_LIMITS).map_err(invalid)?;
        exact_fields(
            &reserve["report"],
            &[
                "format",
                "manifest",
                "executor_configuration",
                "executor_configuration_checksum",
                "sources",
            ],
        )?;
        if reservation != self.record(reservation_id, reserve["report"].clone(), false)
            || header["reservation_checksum"] != crate::bootstrap_evidence::checksum(&reservation)
            || reserve["report"]["format"] != "inventory-qualification-reservation/1"
            || reserve["report"]["manifest"] != self.manifest
            || reserve["report"]["executor_configuration"] != header["executor_configuration"]
            || reserve["report"]["executor_configuration_checksum"]
                != header["executor_configuration_checksum"]
            || reserve["report"]["sources"] != header["sources"]
        {
            return Err(invalid(
                "qualification original pre-admission reservation differs",
            ));
        }
        let refs = self
            .program
            .inputs()
            .instruction()
            .snapshot()
            .revisions()
            .values()
            .map(|revision| revision.reference())
            .collect::<Vec<_>>();
        brassclaw_skills::revision_store::PgComponentRevisionStore::read_exact_in_transaction(
            &tx,
            &[self.program.inputs().instruction().recipe().uuid],
            &refs,
        )
        .await
        .map_err(invalid)?;
        let behavior_bytes = required(&tx, self.behavior).await?;
        let behavior = strict_json(&behavior_bytes, REVISION_LIMITS).map_err(invalid)?;
        exact_fields(
            &behavior["report"],
            &[
                "prerequisites",
                "usage",
                "task_handle",
                "input_checksum",
                "arguments_checksum",
                "answer_checksum",
                "result_checksum",
                "native_answer",
                "preterminal_observation",
                "settled_approval",
            ],
        )?;
        if behavior_bytes != self.record(self.behavior, behavior["report"].clone(), false)
            || behavior["report"]["prerequisites"]["header"] != *header
            || behavior["report"]["preterminal_observation"] != true
            || behavior["report"]["settled_approval"] != false
        {
            return Err(invalid("qualification original native observation differs"));
        }
        let raw_quiescence = required(&tx, self.quiescence).await?;
        let quiescence = strict_json(&raw_quiescence, REVISION_LIMITS).map_err(invalid)?;
        let report = &quiescence["report"];
        exact_fields(
            report,
            &[
                "format",
                "manifest",
                "behavior_checksum",
                "turn_scope",
                "native",
            ],
        )?;
        if raw_quiescence != self.record(self.quiescence, report.clone(), false)
            || report["format"] != "inventory-quiescence-observation/1"
            || report["manifest"] != self.manifest
            || report["behavior_checksum"] != crate::bootstrap_evidence::checksum(&behavior_bytes)
        {
            return Err(invalid(
                "qualification original quiescence observation differs",
            ));
        }
        self.verify_native(&tx, header, &behavior["report"], &report["native"])
            .await?;
        let run: TurnRunId = serde_json::from_value(header["run_id"].clone()).map_err(invalid)?;
        let scope: TurnScope =
            serde_json::from_value(report["turn_scope"].clone()).map_err(invalid)?;
        self.verify_closed_admission(&tx, run, header, &report["turn_scope"])
            .await?;
        let completed_original =
            crate::bootstrap_evidence::read_evidence(&tx, self.completion).await?;
        let retained_turn = completed_original
            .as_deref()
            .map(|bytes| {
                let completed = strict_json(bytes, REVISION_LIMITS).map_err(invalid)?;
                exact_fields(
                    &completed["report"],
                    &[
                        "format",
                        "manifest",
                        "intent_checksum",
                        "behavior_checksum",
                        "quiescence_checksum",
                        "turn_record",
                    ],
                )?;
                let facts = &completed["report"];
                if facts["format"] != "inventory-qualification-completion/1"
                    || facts["manifest"] != self.manifest
                    || facts["intent_checksum"]
                        != crate::bootstrap_evidence::checksum(&intent_bytes)
                    || facts["behavior_checksum"]
                        != crate::bootstrap_evidence::checksum(&behavior_bytes)
                    || facts["quiescence_checksum"]
                        != crate::bootstrap_evidence::checksum(&raw_quiescence)
                    || bytes != self.record(self.completion, facts.clone(), false)
                {
                    return Err(invalid("retained completed original differs"));
                }
                exact_typed::<brassclaw_turns::TurnRunRecord>(&facts["turn_record"])
            })
            .transpose()?;
        // Turn's owner checks out its own client. Release this snapshot first,
        // including for a pool with exactly one connection. V102/V103 retain
        // immutable facts; V179 makes settled Admission rows immutable. Verify
        // those originals again after observing the actual terminal Turn.
        tx.commit().await.map_err(invalid)?;
        client.recycle_after_commit();
        drop(client);
        // The private immutable completion writer below is reached only after
        // real native/VM/admission checks AND this original Turn owner's ACK.
        // Normal Turn snapshot pruning may later remove terminal history. Its
        // trusted V102 completion snapshot remains proof of that past ACK; a
        // missing mutable row alone can never create this record or allow replay.
        let run_record = match retained_turn {
            Some(record) => record,
            None => PgTurnStateStore::new(pool.clone(), scope.tenant_id.as_str())
                .get_run_record(&scope, run)
                .await
                .map_err(invalid)?
                .ok_or_else(|| invalid("qualification original Turn missing before completion"))?,
        };
        if run_record.status != TurnStatus::Completed
            || run_record.run_id != run
            || run_record.scope != scope
            || run_record.claim_count != 1
            || run_record.runner_id.is_some()
            || run_record.lease_token.is_some()
            || run_record.lease_expires_at.is_some()
            || run_record.failure.is_some()
        {
            return Err(invalid("qualification original Turn close unacknowledged"));
        }
        let mut client = primitives::EvidenceClient::acquire(pool).await?;
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(invalid)?;
        for (id, original) in [
            (self.intent, &intent_bytes),
            (self.behavior, &behavior_bytes),
            (self.quiescence, &raw_quiescence),
        ] {
            if required(&tx, id).await? != *original {
                return Err(invalid("qualification original changed across Turn read"));
            }
        }
        brassclaw_skills::revision_store::PgComponentRevisionStore::read_exact_in_transaction(
            &tx,
            &[self.program.inputs().instruction().recipe().uuid],
            &refs,
        )
        .await
        .map_err(invalid)?;
        self.verify_native(&tx, header, &behavior["report"], &report["native"])
            .await?;
        self.verify_closed_admission(&tx, run, header, &report["turn_scope"])
            .await?;
        let completion_bytes=self.record(self.completion,json!({"format":"inventory-qualification-completion/1","manifest":self.manifest,
            "intent_checksum":crate::bootstrap_evidence::checksum(&intent_bytes),"behavior_checksum":crate::bootstrap_evidence::checksum(&behavior_bytes),
            "quiescence_checksum":crate::bootstrap_evidence::checksum(&raw_quiescence),"turn_record":run_record}),false);
        let stored = crate::bootstrap_evidence::read_evidence(&tx, self.completion).await?;
        if stored
            .as_deref()
            .is_some_and(|stored| stored != completion_bytes)
        {
            return Err(invalid("qualification completed original readback differs"));
        }
        tx.commit().await.map_err(invalid)?;
        client.recycle_after_commit();
        drop(client);
        // Known completed original, including true Turn/Admission ACKs. This is
        // metadata recovery only; no admission, worker or Tool is started.
        if stored.is_none() {
            primitives::retain_record(
                pool,
                self.storage.as_ref(),
                self.completion,
                &completion_bytes,
            )
            .await?;
        }
        Ok(Some(InventoryQualification {
            subject: self.clone(),
            intent_bytes,
            behavior_bytes,
            quiescence_bytes: raw_quiescence,
            completion_bytes,
        }))
    }
    async fn verify_closed_admission(
        &self,
        tx: &Transaction<'_>,
        run: TurnRunId,
        header: &Value,
        scope: &Value,
    ) -> Result<(), RebornBuildError> {
        let admission = tx
            .query_one(
                "SELECT phase,outcome,scope,runner_id,claim_checksum FROM brassclaw_monty_task_admissions WHERE run_id=$1",
                &[&run.as_uuid()],
            )
            .await
            .map_err(invalid)?;
        if admission.get::<_, &str>(0) != "settled"
            || admission.get::<_, Option<Value>>(1)
                != Some(json!({"status":"failed","reason_kind":"bootstrap_validation_closed"}))
            || admission.get::<_, Value>(2) != *scope
            || json!(admission.get::<_, Uuid>(3)) != header["runner_id"]
            || hex::encode(admission.get::<_, Vec<u8>>(4)) != header["claim_checksum"]
        {
            return Err(invalid("qualification original admission not fully closed"));
        }
        Ok(())
    }
    async fn verify_native(
        &self,
        tx: &Transaction<'_>,
        header: &Value,
        observed: &Value,
        quiescence: &Value,
    ) -> Result<(), RebornBuildError> {
        let original =
            crate::global_monty_startup::ControlledExecutorConfiguration::restore_original(
                header["executor_configuration"]
                    .as_str()
                    .ok_or_else(|| invalid("original executor configuration missing"))?,
            )?;
        if quiescence["execution"]["controlled_executor_configuration"] != original.exact_bytes()
            || header["executor_configuration_checksum"] != original.checksum()
        {
            return Err(invalid(
                "original executor configuration differs from native execution",
            ));
        }
        let run: Uuid = serde_json::from_value(header["run_id"].clone()).map_err(invalid)?;
        let (step, _) = require_inventory_binding(&self.program)?;
        crate::bootstrap_quiescence_wire::verify(
            quiescence,
            &self.program,
            &observed["task_handle"],
            observed["native_answer"]
                .as_str()
                .ok_or_else(|| invalid("native answer missing"))?,
        )?;
        let count = tx
            .query_one(
                "SELECT count(*) FROM brassclaw_monty_tool_invocations WHERE run_id=$1",
                &[&run],
            )
            .await
            .map_err(invalid)?
            .get::<_, i64>(0);
        if count != 1 {
            return Err(invalid(
                "qualification original has extra or missing native usage",
            ));
        }
        let row=tx.query_one("SELECT s.selection_bytes,s.selection_checksum,i.selection_bytes,i.selection_checksum,i.arguments_bytes,i.arguments_checksum,i.answer_bytes,i.answer_checksum,i.phase,i.attempt_count,i.data_max_depth,i.data_max_nodes,i.data_max_bytes FROM brassclaw_monty_recipe_selections s JOIN brassclaw_monty_tool_invocations i USING(run_id,recipe_id) WHERE s.run_id=$1 AND s.recipe_id=$2 AND i.step_id=$3",
            &[&run,&self.program.inputs().instruction().recipe().uuid,&step]).await.map_err(invalid)?;
        let selection = self
            .program
            .inputs()
            .instruction()
            .retained_selection()
            .map_err(invalid)?;
        let invocation =
            PgMontyAdmission::invocation_selection(&self.program, step).map_err(invalid)?;
        let arguments: &str = row.get(4);
        let answer_bytes = row
            .get::<_, Option<&str>>(6)
            .ok_or_else(|| invalid("original usage answer unresolved"))?;
        if row.get::<_, &str>(0) != selection.exact_bytes()
            || row.get::<_, &str>(1) != hex::encode(selection.checksum())
            || row.get::<_, &str>(2) != invocation
            || row.get::<_, &str>(3) != crate::bootstrap_evidence::checksum(&invocation)
            || row.get::<_, &str>(5) != crate::bootstrap_evidence::checksum(arguments)
            || row.get::<_, Option<&str>>(7)
                != Some(crate::bootstrap_evidence::checksum(answer_bytes).as_str())
            || row.get::<_, &str>(8) != "answered"
            || row.get::<_, i16>(9) != 1
        {
            return Err(invalid("original usage journal differs"));
        }
        let limits = ContractLimits {
            max_depth: usize::try_from(row.get::<_, i64>(10)).map_err(invalid)?,
            max_nodes: usize::try_from(row.get::<_, i64>(11)).map_err(invalid)?,
            max_bytes: usize::try_from(row.get::<_, i64>(12)).map_err(invalid)?,
        };
        let arguments = strict_json(arguments, limits).map_err(invalid)?;
        let answer: PortAnswer =
            serde_json::from_value(strict_json(answer_bytes, limits).map_err(invalid)?)
                .map_err(invalid)?;
        let PortAnswer::Return { value } = &answer else {
            return Err(invalid("original usage native answer failed"));
        };
        let raw = observed["native_answer"]
            .as_str()
            .ok_or_else(|| invalid("original usage native bytes missing"))?;
        if value.as_value() != &json!(raw)
            || arguments != json!({"sources":header["sources"]})
            || observed["input_checksum"]
                != hex::encode(
                    typed_value_checksum_with_limits(&arguments, limits).map_err(invalid)?,
                )
            || observed["arguments_checksum"] != observed["input_checksum"]
            || observed["answer_checksum"]
                != hex::encode(port_answer_checksum_with_limits(&answer, limits).map_err(invalid)?)
            || observed["result_checksum"]
                != hex::encode(
                    typed_value_checksum_with_limits(value.as_value(), limits).map_err(invalid)?,
                )
            || observed["usage"] != strict_json(raw, limits).map_err(invalid)?
        {
            return Err(invalid("original usage fingerprints or fixed input differ"));
        }
        require_fixture(
            &observed["usage"],
            &header["sources"],
            self.worker.checksum(),
        )?;
        self.verify_primitives(tx, header, observed).await
    }
    async fn verify_primitives(
        &self,
        tx: &Transaction<'_>,
        header: &Value,
        observed: &Value,
    ) -> Result<(), RebornBuildError> {
        let records = observed["prerequisites"]["primitive_records"]
            .as_array()
            .filter(|records| records.len() == 4)
            .ok_or_else(|| invalid("complete primitive originals required"))?;
        for (index, name) in [
            "primitive-valid",
            "primitive-invalid-input",
            "primitive-capacity",
            "primitive-denial",
        ]
        .iter()
        .enumerate()
        {
            let id = Uuid::new_v5(&self.intent, name.as_bytes());
            let native_id = Uuid::new_v5(&id, b"native-return");
            let ack_id = Uuid::new_v5(&id, b"outer-host-ack");
            let original = required(tx, id).await?;
            let original_record = strict_json(&original, REVISION_LIMITS).map_err(invalid)?;
            let input = &original_record["report"];
            let ack = required(tx, ack_id).await?;
            let ack_record = strict_json(&ack, REVISION_LIMITS).map_err(invalid)?;
            let outer = &ack_record["report"];
            exact_fields(
                input,
                &[
                    "format",
                    "qualification_intent",
                    "scope",
                    "invocation_id",
                    "capability_id",
                    "arguments",
                    "estimate",
                    "fixture_profile",
                ],
            )?;
            exact_fields(
                outer,
                &[
                    "format",
                    "intent_bytes",
                    "intent_checksum",
                    "native_observation_bytes",
                    "host_outcome",
                    "policy_reads",
                ],
            )?;
            exact_fields(
                &records[index],
                &[
                    "intent_id",
                    "intent_checksum",
                    "native_id",
                    "native_checksum",
                    "ack_id",
                    "ack_checksum",
                ],
            )?;
            let _: brassclaw_host_api::ResourceScope = exact_typed(&input["scope"])?;
            let _: brassclaw_host_api::ResourceEstimate = exact_typed(&input["estimate"])?;
            if original != self.record(id, input.clone(), false)
                || ack != self.record(ack_id, outer.clone(), false)
                || outer["intent_bytes"] != original
                || outer["intent_checksum"] != crate::bootstrap_evidence::checksum(&original)
                || input["qualification_intent"] != json!(self.intent)
                || records[index]["intent_id"] != json!(id)
                || records[index]["ack_id"] != json!(ack_id)
                || records[index]["intent_checksum"]
                    != crate::bootstrap_evidence::checksum(&original)
                || records[index]["ack_checksum"] != crate::bootstrap_evidence::checksum(&ack)
                || input["format"] != "inventory-primitive-intent/1"
                || outer["format"] != "inventory-primitive-host-observation/1"
                || outer["host_outcome"]["capability_id"]
                    != brassclaw_host_runtime::SOURCE_INSPECTION_CAPABILITY_ID
                || (index != 2 && !input["fixture_profile"].is_null())
            {
                return Err(invalid("primitive intent/ack original differs"));
            }
            let key = [
                "primitive_invocation",
                "invalid_primitive_invocation",
                "capacity_primitive_invocation",
                "denied_primitive_invocation",
            ][index];
            let mut scope = header["primitive_scope"].clone();
            scope["invocation_id"] = header[key].clone();
            if input["scope"] != scope
                || input["invocation_id"] != header[key]
                || input["capability_id"] != brassclaw_host_runtime::SOURCE_INSPECTION_CAPABILITY_ID
            {
                return Err(invalid("primitive scope or original invocation differs"));
            }
            let reads = outer["policy_reads"]
                .as_array()
                .filter(|reads| !reads.is_empty())
                .ok_or_else(|| invalid("primitive live policy read missing"))?;
            for read in reads {
                exact_fields(read, &["enabled", "revision", "restricted_by_fixture"])?;
            }
            if reads.iter().any(|read| {
                read["enabled"] != true
                    || read["revision"].as_u64().is_none_or(|r| r == 0)
                    || read["restricted_by_fixture"] != json!(index == 3)
            }) {
                return Err(invalid("real current Tool block or wrong restriction"));
            }
            let native = crate::bootstrap_evidence::read_evidence(tx, native_id).await?;
            exact_fields(
                &outer["host_outcome"],
                if index == 0 {
                    &["kind", "capability_id", "output", "usage"]
                } else {
                    &["kind", "capability_id", "failure_kind", "message"]
                },
            )?;
            if index == 3 {
                if native.is_some()
                    || !outer["native_observation_bytes"].is_null()
                    || !records[index]["native_id"].is_null()
                    || !records[index]["native_checksum"].is_null()
                    || outer["host_outcome"]["kind"] != "failed"
                    || outer["host_outcome"]["failure_kind"] != "authorization"
                    || input["arguments"] != json!({"sources":header["sources"]})
                {
                    return Err(invalid("denial is not original restrictive authorization"));
                }
                continue;
            }
            let native =
                native.ok_or_else(|| invalid("original native primitive observation missing"))?;
            let native_record = strict_json(&native, REVISION_LIMITS).map_err(invalid)?;
            let native_report = &native_record["report"];
            exact_fields(
                native_report,
                &[
                    "format",
                    "intent_checksum",
                    "native_bytes",
                    "native_checksum",
                ],
            )?;
            let native_bytes = native_report["native_bytes"]
                .as_str()
                .ok_or_else(|| invalid("native raw observation missing"))?;
            let facts = strict_json(native_bytes, REVISION_LIMITS).map_err(invalid)?;
            exact_fields(&facts, &["format", "scope", "arguments", "outcome"])?;
            exact_fields(
                &facts["outcome"],
                if index == 0 {
                    &["kind", "output", "usage"]
                } else {
                    &["kind", "dispatch_kind", "safe_summary", "usage"]
                },
            )?;
            let usage: brassclaw_host_api::ResourceUsage = exact_typed(&facts["outcome"]["usage"])?;
            if native != self.record(native_id, native_report.clone(), false)
                || native_report["format"] != "inventory-primitive-native-observation/1"
                || facts["format"] != "source-inspection-dispatch-observation/1"
                || native_report["intent_checksum"]
                    != crate::bootstrap_evidence::checksum(&original)
                || native_report["native_checksum"]
                    != crate::bootstrap_evidence::checksum(native_bytes)
                || outer["native_observation_bytes"] != native
                || facts["scope"] != scope
                || facts["arguments"] != input["arguments"]
                || records[index]["native_checksum"] != crate::bootstrap_evidence::checksum(&native)
                || records[index]["native_id"] != json!(native_id)
            {
                return Err(invalid("primitive native observation original differs"));
            }
            if index == 0 {
                if input["arguments"] != json!({"sources":header["sources"]})
                    || facts["outcome"]["kind"] != "returned"
                    || outer["host_outcome"]["kind"] != "completed"
                    || outer["host_outcome"]["output"] != facts["outcome"]["output"]
                    || outer["host_outcome"]["usage"] != facts["outcome"]["usage"]
                    || facts["outcome"]["usage"]["process_count"] != 1
                {
                    return Err(invalid("primitive positive native completion differs"));
                }
                let report = strict_json(
                    facts["outcome"]["output"]
                        .as_str()
                        .ok_or_else(|| invalid("native primitive output not text"))?,
                    REVISION_LIMITS,
                )
                .map_err(invalid)?;
                require_fixture(&report, &header["sources"], self.worker.checksum())?;
            } else {
                let expected = if index == 1 {
                    brassclaw_host_runtime::SOURCE_INVENTORY_INPUT_INVALID_REASON.to_owned()
                } else {
                    self.verify_fixture_profile(
                        input["fixture_profile"]
                            .as_str()
                            .ok_or_else(|| invalid("fixture original profile missing"))?,
                    )?
                };
                if facts["outcome"]["kind"] != "rejected"
                    || facts["outcome"]["safe_summary"] != expected
                    || usage != brassclaw_host_api::ResourceUsage::default()
                    || outer["host_outcome"]["kind"] != "failed"
                    || outer["host_outcome"]["message"] != expected
                    || outer["host_outcome"]["failure_kind"]
                        != if index == 1 {
                            "invalid_input"
                        } else {
                            "resource"
                        }
                    || facts["outcome"]["dispatch_kind"]
                        != if index == 1 {
                            "input_encode"
                        } else {
                            "resource"
                        }
                {
                    return Err(invalid("negative native fixture has unrelated failure"));
                }
                if index == 1 && input["arguments"] != json!({"sources":{"":""}}) {
                    return Err(invalid("invalid-input fixture arguments differ"));
                }
                if index == 2
                    && (input["arguments"]
                        != brassclaw_host_runtime::SourceInspectionFixtureProfile::package_arguments(
                        )
                        || input["fixture_profile"] != header["capacity_fixture_profile"]
                        || header["capacity_fixture_checksum"]
                            != crate::bootstrap_evidence::checksum(
                                input["fixture_profile"]
                                    .as_str()
                                    .ok_or_else(|| invalid("capacity original profile missing"))?,
                            ))
                {
                    return Err(invalid("capacity fixture is not fixed package bytes"));
                }
            }
        }
        Ok(())
    }
    fn verify_fixture_profile(&self, bytes: &str) -> Result<String, RebornBuildError> {
        let profile = strict_json(bytes, REVISION_LIMITS).map_err(invalid)?;
        exact_fields(
            &profile,
            &[
                "format",
                "profile_id",
                "identity_material_bytes",
                "identity_material_checksum",
            ],
        )?;
        let material = profile["identity_material_bytes"]
            .as_str()
            .ok_or_else(|| invalid("fixture identity material missing"))?;
        let identity = crate::bootstrap_evidence::checksum(material);
        let data = strict_json(material, REVISION_LIMITS).map_err(invalid)?;
        exact_fields(
            &data,
            &[
                "format",
                "fixture_revision",
                "base_origin",
                "base_revision",
                "base_limits_bytes",
                "base_limits_checksum",
                "fixture_limits_bytes",
                "fixture_limits_checksum",
                "fixed_arguments_bytes",
                "fixed_arguments_checksum",
                "worker_checksum",
                "native_implementation",
                "fixture_definition_checksum",
                "owner_code_checksum",
                "adapter_code_checksum",
                "backend_binding_id",
            ],
        )?;
        if profile.as_object().is_none_or(|o| o.len() != 4)
            || profile["format"] != "source-inspection-fixture-profile/1"
            || profile["profile_id"] != identity
            || profile["identity_material_checksum"] != identity
            || data["format"] != "source-inspection-fixture-identity/1"
            || data["native_implementation"] != self.manifest["native"]
            || data["worker_checksum"] != self.manifest["worker_checksum"]
            || data["owner_code_checksum"]
                != hex::encode(
                    serde_json::from_value::<brassclaw_host_runtime::NativeImplementationRef>(
                        self.manifest["native"].clone(),
                    )
                    .map_err(invalid)?
                    .artifact_checksum(),
                )
            || !matches!(
                data["base_origin"].as_str(),
                Some("controlled_bootstrap" | "acknowledged_live")
            )
            || data["fixture_revision"] != 1
            || data["base_revision"].as_u64().is_none_or(|r| r == 0)
        {
            return Err(invalid("fixture original identity/provenance differs"));
        }
        let base_bytes = data["base_limits_bytes"]
            .as_str()
            .ok_or_else(|| invalid("fixture base limits missing"))?;
        let limit_bytes = data["fixture_limits_bytes"]
            .as_str()
            .ok_or_else(|| invalid("fixture derived limits missing"))?;
        let arguments = brassclaw_host_runtime::SourceInspectionFixtureProfile::package_arguments();
        let argument_bytes = serde_json::to_string(&arguments).map_err(invalid)?;
        let source = arguments["sources"]["over_capacity"]
            .as_str()
            .ok_or_else(|| invalid("package capacity source missing"))?;
        let mut limits: brassclaw_host_api::MontyExecutionLimits =
            serde_json::from_value(strict_json(base_bytes, REVISION_LIMITS).map_err(invalid)?)
                .map_err(invalid)?;
        limits.validate().map_err(invalid)?;
        if serde_json::to_string(&limits).map_err(invalid)? != base_bytes {
            return Err(invalid("fixture parent bytes normalized"));
        }
        limits.max_source_bytes = limits.max_source_bytes.min(
            u64::try_from(
                source
                    .len()
                    .checked_sub(1)
                    .ok_or_else(|| invalid("empty capacity source"))?,
            )
            .map_err(invalid)?,
        );
        if serde_json::to_string(&limits).map_err(invalid)? != limit_bytes
            || data["base_limits_checksum"] != crate::bootstrap_evidence::checksum(base_bytes)
            || data["fixture_limits_checksum"] != crate::bootstrap_evidence::checksum(limit_bytes)
            || data["fixed_arguments_bytes"] != argument_bytes
            || data["fixed_arguments_checksum"]
                != crate::bootstrap_evidence::checksum(&argument_bytes)
            || data["fixture_definition_checksum"] != crate::bootstrap_evidence::checksum(source)
            || data["adapter_code_checksum"]
                != crate::bootstrap_evidence::checksum(include_str!(
                    "../../brassclaw_host_runtime/src/source_inspection_fixture.rs"
                ))
            || serde_json::from_value::<Uuid>(data["backend_binding_id"].clone())
                .map_err(invalid)?
                .is_nil()
        {
            return Err(invalid("fixture actual bounded profile differs"));
        }
        Ok(format!(
            "source inventory fixture source byte limit exceeded profile {identity} fixture_revision 1 base_revision {} limit {} actual {}",
            data["base_revision"]
                .as_u64()
                .ok_or_else(|| invalid("fixture revision missing"))?,
            limits.max_source_bytes,
            source.len()
        ))
    }
}
fn exact_fields(value: &Value, keys: &[&str]) -> Result<(), RebornBuildError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("original observation object required"))?;
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(invalid("original observation required fields differ"));
    }
    Ok(())
}
fn exact_typed<T: serde::de::DeserializeOwned + serde::Serialize>(
    value: &Value,
) -> Result<T, RebornBuildError> {
    let decoded: T = serde_json::from_value(value.clone()).map_err(invalid)?;
    if serde_json::to_value(&decoded).map_err(invalid)? != *value {
        return Err(invalid("original typed observation normalized"));
    }
    Ok(decoded)
}
async fn required(tx: &Transaction<'_>, id: Uuid) -> Result<String, RebornBuildError> {
    crate::bootstrap_evidence::read_evidence(tx, id)
        .await?
        .ok_or_else(|| invalid("original qualification incomplete; reconcile without replay"))
}

pub(super) struct InventoryQualification {
    subject: Arc<Subject>,
    intent_bytes: String,
    behavior_bytes: String,
    quiescence_bytes: String,
    completion_bytes: String,
}
impl InventoryQualification {
    pub(in crate::validator_inventory_review) fn record_reference(
        &self,
    ) -> brassclaw_skills::review_submission_store::installation::InstallationRecordRef {
        brassclaw_skills::review_submission_store::installation::InstallationRecordRef {
            id: self.subject.completion,
            checksum: crate::bootstrap_evidence::checksum(&self.completion_bytes),
        }
    }
    pub(in crate::validator_inventory_review) async fn verify(
        &self,
        pool: &Arc<PgPool>,
    ) -> Result<(), RebornBuildError> {
        let read = self
            .subject
            .read(pool)
            .await?
            .ok_or_else(|| invalid("qualification original missing"))?;
        if self.intent_bytes != read.intent_bytes
            || self.behavior_bytes != read.behavior_bytes
            || self.quiescence_bytes != read.quiescence_bytes
            || self.completion_bytes != read.completion_bytes
        {
            return Err(invalid("qualification original exact readback differs"));
        }
        Ok(())
    }
}
