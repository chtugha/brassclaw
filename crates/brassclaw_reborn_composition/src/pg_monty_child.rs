// Installation catalogue child provenance; not part of the independent transport shim.
impl PgMontyAdmission {
    /// Establish native child provenance from this original private admission,
    /// committed invocation and pinned catalogue. Request fields are data, never
    /// authority. This read-only check admits nothing and cannot repair a claim.
    pub(crate) async fn verify_child_invocation(
        &self,
        invocation_id: brassclaw_host_api::InvocationId,
        recipe: uuid::Uuid,
        variant: &str,
        inputs: &serde_json::Value,
        catalogue: CatalogueReference,
    ) -> Result<DateTime<Utc>, AgentLoopDriverError> {
        let mut client = self
            .pool
            .get()
            .await
            .map_err(|_| failed("child_provenance_unavailable"))?;
        let tx = client
            .transaction()
            .await
            .map_err(|_| failed("child_provenance_unavailable"))?;
        self.verify_claim(&tx).await?;
        if self.catalogue.get() != Some(&Some(catalogue)) {
            return Err(failed("child_provenance_invalid"));
        }
        let row = tx.query_opt(
            "SELECT i.selection_bytes,i.selection_checksum,i.arguments_bytes,i.arguments_checksum,
                    i.recorded_at,i.phase,a.catalogue_id,a.catalogue_checksum
             FROM brassclaw_monty_tool_invocations i
             JOIN brassclaw_monty_task_admissions a USING(run_id)
             WHERE i.invocation_id=$1 AND i.run_id=$2 AND i.admission_key=$3
               AND a.admission_key=$3 AND a.claim_checksum=$4 AND a.phase='started'",
            &[&invocation_id.as_uuid(), &self.attempt.run_id.as_uuid(), &&self.key[..], &&self.checksum[..]],
        ).await.map_err(|_| failed("child_provenance_unavailable"))?
            .ok_or_else(|| failed("child_provenance_invalid"))?;
        let selection_bytes: &str = row.get(0);
        let arguments_bytes: &str = row.get(2);
        let selection: brassclaw_host_api::JsonValue = serde_json::from_str(selection_bytes)
            .map_err(|_| failed("child_provenance_invalid"))?;
        let arguments: brassclaw_host_api::JsonValue = serde_json::from_str(arguments_bytes)
            .map_err(|_| failed("child_provenance_invalid"))?;
        if invocation::digest(selection_bytes) != row.get::<_, &str>(1)
            || invocation::digest(arguments_bytes) != row.get::<_, &str>(3)
            || selection["capability_id"] != brassclaw_host_runtime::SPAWN_SUBAGENT_CAPABILITY_ID
            || row.get::<_, &str>(5) != "dispatch_intent"
            || row.get::<_, Option<uuid::Uuid>>(6) != Some(catalogue.id)
            || row.get::<_, Option<String>>(7).as_deref()
                != Some(hex::encode(catalogue.checksum).as_str())
            || arguments.as_object().is_none_or(|object| object.len() != 2)
            || arguments["workflow"]
                .as_object()
                .is_none_or(|object| object.len() != 2)
            || arguments["workflow"]["recipe_id"].as_str() != Some(recipe.to_string().as_str())
            || arguments["workflow"]["variant"].as_str() != Some(variant)
            || &arguments["inputs"] != inputs
        {
            return Err(failed("child_provenance_invalid"));
        }
        let received_at = row.get(4);
        tx.commit()
            .await
            .map_err(|_| failed("child_provenance_unavailable"))?;
        Ok(received_at)
    }
}
