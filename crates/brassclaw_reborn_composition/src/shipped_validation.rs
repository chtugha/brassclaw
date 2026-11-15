//! Installation transfers validation of canonical shipped definitions. It does
//! not run qualification fixtures or claim observations from this instance.
//! The private scope is entered only by the packaged catalogue constructor;
//! authored inputs and stored source labels cannot select it.
use crate::{
    RebornBuildError, bootstrap_evidence as evidence, bootstrap_reply_validation::invalid,
};
use brassclaw_pg::PgPool;
use brassclaw_skills::{
    association_contract::AssociationApprovalDeclaration, component_revision::REVISION_LIMITS,
};
use serde_json::{Value, json};
use tokio_postgres::Transaction;
use uuid::Uuid;

tokio::task_local! { static INSTALLATION: (); }
#[cfg(test)]
tokio::task_local! { static DEVELOPMENT_QUALIFICATION: (); }

/// Explicit native development qualification, never a product setting or an
/// authored flag. These tests exercise the original fixture/journal oracles.
#[cfg(test)]
pub(crate) async fn qualify_package<T>(future: impl std::future::Future<Output = T>) -> T {
    DEVELOPMENT_QUALIFICATION.scope((), future).await
}
#[cfg(test)]
#[path = "shipped_validation_tests.rs"]
mod tests;

pub(super) async fn installation<T>(future: impl std::future::Future<Output = T>) -> T {
    #[cfg(test)]
    if DEVELOPMENT_QUALIFICATION.try_with(|()| ()).is_ok() {
        return future.await;
    }
    INSTALLATION.scope((), future).await
}

pub(super) fn transferring() -> bool {
    INSTALLATION.try_with(|()| ()).is_ok()
}

pub(super) async fn verify_program(
    tx: &Transaction<'_>,
    program: &brassclaw_engine::executor::retained_source::InspectedRetainedProgram,
) -> Result<(), RebornBuildError> {
    verify_instruction(tx, program.program().inputs().instruction()).await
}

pub(super) async fn verify_instruction(
    tx: &Transaction<'_>,
    instruction: &brassclaw_engine::memory::retained_instruction::RetainedRecipeInstruction,
) -> Result<(), RebornBuildError> {
    let refs = instruction
        .snapshot()
        .revisions()
        .values()
        .map(|r| r.reference())
        .collect::<Vec<_>>();
    brassclaw_skills::revision_store::PgComponentRevisionStore::read_exact_in_transaction(
        tx,
        &[instruction.recipe().uuid],
        &refs,
        brassclaw_skills::revision_store::SnapshotReadLimits::legacy_graph(),
    )
    .await
    .map_err(invalid)?;
    Ok(())
}

/// Exact package identity supplied by a canonical definition owner, never by an
/// authored request. The record explicitly identifies the validation's origin.
#[derive(Clone)]
pub(super) struct Transfer {
    identity: Value,
    id: Uuid,
}

#[derive(Clone)]
pub(super) struct Components {
    transfer: Transfer,
    references: Vec<brassclaw_skills::association_contract::ComponentRevisionRef>,
}

impl Components {
    pub(super) async fn install(
        pool: std::sync::Arc<PgPool>,
        drafts: Vec<brassclaw_skills::component_revision::ComponentRevisionDraft>,
    ) -> Result<Self, RebornBuildError> {
        if !transferring() {
            return Err(invalid("component transfer outside package installation"));
        }
        let store = brassclaw_skills::revision_store::PgComponentRevisionStore::new(pool.clone());
        let mut references = Vec::new();
        for draft in drafts {
            references.push(store.retain_packaged_draft(&draft).await.map_err(invalid)?);
        }
        references.sort_by_key(|r| r.uuid);
        if references
            .windows(2)
            .any(|pair| pair[0].uuid == pair[1].uuid && pair[0] != pair[1])
        {
            return Err(invalid("conflicting shipped component definitions"));
        }
        references.dedup();
        let identity = references
            .iter()
            .map(|r| {
                json!({"uuid":r.uuid,"class_code":r.class_code,
            "version":r.version,"checksum":hex::encode(r.checksum)})
            })
            .collect::<Vec<_>>();
        let transfer = Transfer::new(json!({"components":identity}));
        transfer.install(&pool).await?;
        Ok(Self {
            transfer,
            references,
        })
    }

    pub(super) async fn verify(&self, tx: &Transaction<'_>) -> Result<Value, RebornBuildError> {
        self.transfer.require(tx).await?;
        for reference in &self.references {
            brassclaw_skills::revision_store::PgComponentRevisionStore::read_revision_in_transaction(tx, *reference)
                .await.map_err(invalid)?;
        }
        Ok(self.transfer.reference())
    }

    pub(super) fn require_program(
        &self,
        program: &brassclaw_engine::memory::retained_tools::RetainedToolProgram,
    ) -> Result<(), RebornBuildError> {
        let graph = program.inputs().instruction().snapshot().revisions();
        for reference in &self.references {
            if graph
                .get(&reference.uuid)
                .is_none_or(|r| r.reference() != *reference)
            {
                return Err(invalid(
                    "selected component differs from shipped validation",
                ));
            }
        }
        Ok(())
    }
}

impl Transfer {
    pub(super) fn id(&self) -> Uuid {
        self.id
    }
    pub(super) fn checksum(&self) -> String {
        evidence::checksum(&self.bytes())
    }
    pub(super) fn new(identity: Value) -> Self {
        let id = Uuid::new_v5(
            &Uuid::from_u128(0x34bc1a79_b082_4ac2_a776_763cd818f083),
            identity.to_string().as_bytes(),
        );
        Self { identity, id }
    }

    pub(super) fn reference(&self) -> Value {
        json!({"id":self.id,"checksum":evidence::checksum(&self.bytes())})
    }

    fn q1_id(&self) -> Uuid {
        Uuid::new_v5(&self.id, b"inherited-q1")
    }

    fn record(&self, id: Uuid, kind: &str) -> String {
        evidence::system_record(
            id,
            kind,
            true,
            &evidence::checksum(&self.identity.to_string()),
            &[],
            json!({"format":"shipped-validation-transfer/1", "validation_origin":"provisioning",
                "installation_execution":false,"identity":self.identity}),
        )
    }

    fn bytes(&self) -> String {
        self.record(self.id, "behavior")
    }

    pub(super) async fn retain(&self, tx: &Transaction<'_>) -> Result<(), RebornBuildError> {
        if !transferring() {
            return Err(invalid(
                "shipped validation can only be transferred during package installation",
            ));
        }
        evidence::require_coherent(tx).await?;
        evidence::retain_intent(tx, self.q1_id(), &self.record(self.q1_id(), "q1")).await?;
        evidence::retain_intent(tx, self.id, &self.bytes()).await?;
        self.require(tx).await
    }

    pub(super) async fn present(&self, tx: &Transaction<'_>) -> Result<bool, RebornBuildError> {
        evidence::require_coherent(tx).await?;
        let q1 = evidence::read_evidence(tx, self.q1_id()).await?;
        let behavior = evidence::read_evidence(tx, self.id).await?;
        match (q1, behavior) {
            (None, None) => Ok(false),
            (Some(q1), Some(behavior))
                if q1 == self.record(self.q1_id(), "q1") && behavior == self.bytes() =>
            {
                Ok(true)
            }
            _ => Err(invalid("shipped validation transfer missing or changed")),
        }
    }

    pub(super) async fn require(&self, tx: &Transaction<'_>) -> Result<(), RebornBuildError> {
        if !self.present(tx).await? {
            return Err(invalid("shipped validation transfer missing"));
        }
        Ok(())
    }

    pub(super) async fn install(&self, pool: &PgPool) -> Result<(), RebornBuildError> {
        let mut client = evidence::io::acquire(pool).await?;
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .start()
            .await
            .map_err(invalid)?;
        self.retain(&tx).await?;
        tx.commit().await.map_err(invalid)?;
        client.recycle_after_commit();
        Ok(())
    }

    /// Keep association identity/coverage, using a separate approval namespace
    /// and references to transferred validation rather than local test results.
    pub(super) fn association(
        &self,
        template: &str,
    ) -> Result<AssociationApprovalDeclaration, RebornBuildError> {
        let mut value: Value = serde_json::from_str(template).map_err(invalid)?;
        if value["validation_mode"] != "system_seed" || !value["q2_ref"].is_null() {
            return Err(invalid(
                "only canonical shipped associations can be transferred",
            ));
        }
        value["approval_id"] = json!(Uuid::new_v5(&self.id, template.as_bytes()));
        value["q1_ref"] = json!(self.q1_id());
        value["behavioral_refs"] = json!([self.id]);
        AssociationApprovalDeclaration::from_json(&value.to_string(), REVISION_LIMITS)
            .map_err(invalid)
    }

    pub(super) async fn retain_association(
        &self,
        tx: &Transaction<'_>,
        template: &str,
    ) -> Result<AssociationApprovalDeclaration, RebornBuildError> {
        self.retain(tx).await?;
        let declaration = self.association(template)?;
        let bytes = declaration.exact_bytes();
        let id = declaration.approval_id();
        tx.execute("INSERT INTO reborn_skill_association_approvals(approval_id,approval_bytes,checksum) VALUES($1,$2,$3) ON CONFLICT(approval_id) DO NOTHING",
            &[&id,&bytes,&evidence::checksum(bytes)]).await.map_err(invalid)?;
        if evidence::read_association(tx, id).await?.as_deref() != Some(bytes) {
            return Err(invalid("transferred shipped association differs"));
        }
        Ok(declaration)
    }

    pub(super) async fn read_association(
        &self,
        tx: &Transaction<'_>,
        template: &str,
    ) -> Result<Option<AssociationApprovalDeclaration>, RebornBuildError> {
        if !self.present(tx).await? {
            return Ok(None);
        }
        let declaration = self.association(template)?;
        if evidence::read_association(tx, declaration.approval_id())
            .await?
            .as_deref()
            != Some(declaration.exact_bytes())
        {
            return Err(invalid(
                "transferred shipped association missing or changed",
            ));
        }
        Ok(Some(declaration))
    }
}
