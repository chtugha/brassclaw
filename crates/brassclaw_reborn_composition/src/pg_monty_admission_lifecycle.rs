// Production lifecycle methods; shared admission/journal enforcement lives in
// pg_monty_admission_core.rs. Included in the same module to retain private
// admission-key custody without exposing fields to sibling adapters.
impl PgMontyAdmission {
    /// Read-only parent admission/claim verification for dependent scheduling.
    /// It starts no operation and cannot restore a fenced or expired attempt.
    pub(crate) async fn check_live_dependency(&self) -> Result<(), AgentLoopDriverError> {
        let mut client = self
            .pool
            .get()
            .await
            .map_err(|_| failed("monty_admission_database_failed"))?;
        let transaction = client
            .transaction()
            .await
            .map_err(|_| failed("monty_admission_database_failed"))?;
        self.verify_claim(&transaction).await?;
        let active: bool = transaction
            .query_one(
                "SELECT EXISTS(SELECT 1
            FROM brassclaw_monty_task_admissions WHERE run_id=$1 AND admission_key=$2
                AND claim_checksum=$3 AND phase='started')",
                &[
                    &self.attempt.run_id.as_uuid(),
                    &&self.key[..],
                    &&self.checksum[..],
                ],
            )
            .await
            .map_err(|_| failed("monty_admission_database_failed"))?
            .get(0);
        if !active {
            return Err(failed("monty_admission_fenced"));
        }
        transaction
            .commit()
            .await
            .map_err(|_| failed("monty_admission_database_failed"))
    }

    /// Distinct from VM settlement: the trusted factory retains real service
    /// proof that this attempt was never enqueued. Only an unstarted reservation
    /// with no execution records may close here. Unknown writes retain the same
    /// private address for an exact idempotent acknowledgement retry.
    pub(crate) async fn settle_unsubmitted(
        &self,
        reason: brassclaw_monty_host::service::UnsubmittedReason,
    ) -> Result<(), AgentLoopDriverError> {
        let outcome = serde_json::json!({"status":"failed","reason_kind":reason.reason_kind()});
        let mut client = self
            .pool
            .get()
            .await
            .map_err(|_| failed("monty_admission_database_failed"))?;
        let transaction = client
            .transaction()
            .await
            .map_err(|_| failed("monty_admission_database_failed"))?;
        let row = transaction
            .query_opt(
                "SELECT phase,started_at,outcome FROM brassclaw_monty_task_admissions
             WHERE run_id=$1 AND admission_key=$2 AND claim_checksum=$3 FOR UPDATE",
                &[
                    &self.attempt.run_id.as_uuid(),
                    &&self.key[..],
                    &&self.checksum[..],
                ],
            )
            .await
            .map_err(|_| failed("monty_admission_database_failed"))?
            .ok_or_else(|| failed("monty_admission_fenced"))?;
        let phase: &str = row.get(0);
        if row.get::<_, Option<DateTime<Utc>>>(1).is_some()
            || (phase != "reserved"
                && (phase != "settled"
                    || row.get::<_, Option<Value>>(2).as_ref() != Some(&outcome)))
        {
            return Err(failed("monty_unsubmitted_identity_conflict"));
        }
        let recorded: bool = transaction
            .query_one(
                "SELECT EXISTS(SELECT 1 FROM brassclaw_monty_recipe_selections WHERE run_id=$1)
                OR EXISTS(SELECT 1 FROM brassclaw_monty_tool_invocations WHERE run_id=$1)",
                &[&self.attempt.run_id.as_uuid()],
            )
            .await
            .map_err(|_| failed("monty_admission_database_failed"))?
            .get(0);
        if recorded {
            return Err(failed("monty_unsubmitted_identity_conflict"));
        }
        if phase == "reserved" {
            let changed = transaction
                .execute(
                    "UPDATE brassclaw_monty_task_admissions
                 SET phase='settled',outcome=$3,settled_at=clock_timestamp()
                 WHERE run_id=$1 AND admission_key=$2 AND phase='reserved' AND started_at IS NULL",
                    &[&self.attempt.run_id.as_uuid(), &&self.key[..], &outcome],
                )
                .await
                .map_err(|_| failed("monty_admission_database_failed"))?;
            if changed != 1 {
                return Err(failed("monty_admission_fenced"));
            }
        }
        transaction
            .commit()
            .await
            .map_err(|_| failed("monty_admission_database_failed"))
    }
}
