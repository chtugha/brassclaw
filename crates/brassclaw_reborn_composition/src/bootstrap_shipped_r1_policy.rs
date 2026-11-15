//! Original Capture1 framing for the closed shipped-Pure installation owner.
//! Integrity and transfer policy only: a frame cannot establish acknowledgement,
//! successful execution, provenance, approval or publication.
use super::*;
use brassclaw_host_api::task_preparation::RetainedPreparation;
use brassclaw_pg::wire_capacity as wire;
use brassclaw_skills::{component_revision::revision_limits, revision_store::SnapshotReadLimits};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ParserLimits {
    // Canonical JSON key order, also used by the borrowed header reader.
    max_bytes: usize,
    max_depth: usize,
    max_nodes: usize,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Frame {
    preparation_bytes: String,
    preparation_checksum: String,
    record_parser_limits: ParserLimits,
}
#[derive(Clone)]
pub(crate) struct OriginalPolicy {
    original: RetainedPreparation,
    frame: Frame,
    limits: ContractLimits,
}

// Intrinsic schema width, independent of the settings being recovered. Capture1
// has only numeric fields and fixed ASCII tags. At most two outer bytes encode
// each character; this is a serializer bound, never an operational quota.
fn maximum_frame_bytes() -> Result<usize, RebornBuildError> {
    let capture = RetainedPreparation::maximum_canonical_bytes().map_err(invalid)?;
    let value = json!({"preparation_bytes":"\"".repeat(capture),
        "preparation_checksum":"0".repeat(64),"record_parser_limits":{
            "max_depth":usize::MAX,"max_nodes":usize::MAX,"max_bytes":usize::MAX}});
    usize::try_from(brassclaw_common::json::count_json_bytes(&value, None).map_err(invalid)?)
        .map_err(invalid)
}

impl OriginalPolicy {
    pub(super) fn new(original: &RetainedPreparation) -> Result<Self, RebornBuildError> {
        original.require_canonical().map_err(invalid)?;
        // R1 is specifically the existing Capture1 contract. Check the exact
        // canonical owner's format prefix as well, so adding another capture
        // version to that codec cannot silently widen this record domain.
        if !original
            .bytes()
            .starts_with("{\"format\":\"task-preparation-capture/1\",")
        {
            return Err(invalid("shipped R1 requires an original Capture1"));
        }
        // revision_limits owns the actual Capture1 parser contract, including
        // its supported depth. Do not replace it with today's defaults.
        let limits = revision_limits(Some(original)).map_err(invalid)?;
        limits.validate_binding_limits().map_err(invalid)?;
        SnapshotReadLimits::captured(original).map_err(invalid)?;
        crate::installed_monty_catalogue::PreparationCapture::from_original(original.clone())
            .map_err(invalid)?;
        Ok(Self {
            original: original.clone(),
            frame: Frame {
                preparation_bytes: original.bytes().into(),
                preparation_checksum: original.checksum().into(),
                record_parser_limits: ParserLimits {
                    max_depth: limits.max_depth,
                    max_nodes: limits.max_nodes,
                    max_bytes: limits.max_bytes,
                },
            },
            limits,
        })
    }
    fn from_frame(raw: &str) -> Result<Self, RebornBuildError> {
        if raw.len() > maximum_frame_bytes()? {
            return Err(invalid("shipped R1 frame exceeds its intrinsic schema"));
        }
        // Deny unknown/duplicate fields at both typed object levels. Exact
        // re-encoding also rejects whitespace, escaped aliases and duplicate
        // keys inside the nested capture, whose older codec stays unchanged.
        let frame: Frame = serde_json::from_str(raw).map_err(invalid)?;
        if frame.preparation_bytes.len()
            > RetainedPreparation::maximum_canonical_bytes().map_err(invalid)?
        {
            return Err(invalid("shipped R1 capture exceeds its intrinsic schema"));
        }
        let original = RetainedPreparation::restore_original(
            &frame.preparation_bytes,
            &frame.preparation_checksum,
        )
        .map_err(invalid)?;
        let policy = Self::new(&original)?;
        // Compare both closed typed fields and exact canonical bytes. JSON
        // value equality alone would accept whitespace or escaped aliases.
        let canonical = serde_json::to_string(&policy.frame).map_err(invalid)?;
        if frame != policy.frame || canonical != raw {
            return Err(invalid(
                "shipped R1 frame differs from its canonical original",
            ));
        }
        Ok(policy)
    }
    pub(crate) fn limits(&self) -> ContractLimits {
        self.limits
    }
    pub(super) fn original(&self) -> &RetainedPreparation {
        &self.original
    }
    pub(crate) fn graph(&self) -> Result<SnapshotReadLimits, RebornBuildError> {
        SnapshotReadLimits::captured(&self.original).map_err(invalid)
    }
    pub(super) fn remove_frame(&self, report: &mut Value) -> Result<(), RebornBuildError> {
        let map = report
            .as_object_mut()
            .ok_or_else(|| invalid("shipped R1 report object missing"))?;
        if map.remove("_frame") != Some(serde_json::to_value(&self.frame).map_err(invalid)?) {
            return Err(invalid("shipped R1 original frame differs or is missing"));
        }
        Ok(())
    }
    pub(super) fn encode(
        &self,
        id: Uuid,
        kind: &str,
        success: bool,
        subject: &str,
        components: &[Value],
        mut report: Value,
    ) -> Result<String, RebornBuildError> {
        let map = report
            .as_object_mut()
            .ok_or_else(|| invalid("shipped R1 report object missing"))?;
        if map
            .insert(
                "_frame".into(),
                serde_json::to_value(&self.frame).map_err(invalid)?,
            )
            .is_some()
        {
            return Err(invalid("shipped R1 report already contains a frame"));
        }
        if map.keys().next().map(String::as_str) != Some("_frame") {
            return Err(invalid("shipped R1 frame must be the first report field"));
        }
        let view = evidence::system_record_view(id, kind, success, subject, components, &report);
        let count = brassclaw_common::json::count_json_bytes(
            &view,
            Some(u64::try_from(self.limits.max_bytes).map_err(invalid)?),
        )
        .map_err(invalid)?;
        let length = usize::try_from(count).map_err(invalid)?;
        check_transfer(length)?;
        if wire::unnamed_typed_request_bytes(evidence::INTENT_INSERT.len(), &[16, length, 64])
            .is_none()
        {
            return Err(invalid("shipped R1 record request is not PG-representable"));
        }
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(length).map_err(invalid)?;
        brassclaw_common::json::write_json(&mut bytes, &view).map_err(invalid)?;
        let bytes = String::from_utf8(bytes).map_err(invalid)?;
        strict_json(&bytes, self.limits).map_err(invalid)?;
        Ok(bytes)
    }
    pub(crate) async fn read(
        &self,
        tx: &Transaction<'_>,
        id: Uuid,
    ) -> Result<Option<String>, RebornBuildError> {
        let Some((length, digest)) = metadata(tx, id).await? else {
            return Ok(None);
        };
        if length > self.limits.max_bytes {
            return Err(invalid(
                "shipped R1 record exceeds its original parser policy",
            ));
        }
        let maximum = i32::try_from(length).map_err(invalid)?;
        let row = tx.query_one(
            "SELECT CASE WHEN octet_length(evidence_bytes)=$2 THEN evidence_bytes END FROM reborn_component_review_evidence WHERE evidence_id=$1",
            &[&id, &maximum],
        ).await.map_err(database)?;
        let bytes: String = row
            .get::<_, Option<String>>(0)
            .ok_or_else(|| invalid("shipped R1 original length changed"))?;
        if bytes.len() != length || evidence::checksum(&bytes) != digest {
            return Err(invalid("shipped R1 original record integrity differs"));
        }
        let mut value = strict_json(&bytes, self.limits).map_err(invalid)?;
        self.remove_frame(&mut value["report"])?;
        Ok(Some(bytes))
    }
    pub(super) async fn retain(
        &self,
        pool: &PgPool,
        storage: &dyn brassclaw_resources::PgResourceStoreSettingsSource,
        id: Uuid,
        bytes: &str,
        intent: bool,
    ) -> Result<(), RebornBuildError> {
        strict_json(bytes, self.limits).map_err(invalid)?;
        check_transfer(bytes.len())?;
        if wire::unnamed_typed_request_bytes(evidence::INTENT_INSERT.len(), &[16, bytes.len(), 64])
            .is_none()
        {
            return Err(invalid("shipped R1 write exceeds PG representation"));
        }
        let write = io::bounded_for(storage, "shipped_r1_write", id, async {
            let mut client = io::acquire(pool).await?;
            let tx = client
                .build_transaction()
                .isolation_level(IsolationLevel::RepeatableRead)
                .start()
                .await
                .map_err(database)?;
            let won = tx
                .execute(
                    evidence::INTENT_INSERT,
                    &[&id, &bytes, &evidence::checksum(bytes)],
                )
                .await
                .map_err(database)?
                == 1;
            if intent && !won {
                return Err(invalid(
                    "shipped R1 intent exists; reconcile without replay",
                ));
            }
            tx.commit().await.map_err(database)?;
            client.recycle_after_commit();
            Ok(())
        })
        .await;
        let readback = io::bounded_for(storage, "shipped_r1_write_readback", id, async {
            let mut client = io::acquire(pool).await?;
            let tx = client
                .build_transaction()
                .isolation_level(IsolationLevel::RepeatableRead)
                .read_only(true)
                .start()
                .await
                .map_err(database)?;
            if self.read(&tx, id).await?.as_deref() != Some(bytes) {
                return Err(invalid(
                    "shipped R1 original write differs or is unresolved",
                ));
            }
            tx.commit().await.map_err(database)?;
            client.recycle_after_commit();
            Ok(())
        })
        .await;
        match (write, readback) {
            (Err(write), Err(readback)) => Err(RebornBuildError::OriginalShippedPure {
                source: Box::new(UnresolvedWrite { write, readback }),
            }),
            // An unacknowledged first intent never authorizes execution, even
            // when readback sees the row. Only later original reconciliation.
            (Err(write), _) if intent => Err(write),
            (_, readback) => readback,
        }
    }
}

#[derive(thiserror::Error)]
#[error("original shipped write and readback are unresolved")]
struct UnresolvedWrite {
    #[source]
    write: RebornBuildError,
    readback: RebornBuildError,
}
impl std::fmt::Debug for UnresolvedWrite {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Keep both original causes without dumping private database payloads.
        f.debug_struct("UnresolvedWrite")
            .field("write", &self.write.to_string())
            .field("readback", &self.readback.to_string())
            .finish()
    }
}
struct DatabaseCause(tokio_postgres::Error);
impl std::fmt::Debug for DatabaseCause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DatabaseCause")
    }
}
impl std::fmt::Display for DatabaseCause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("shipped R1 database operation failed")
    }
}
impl std::error::Error for DatabaseCause {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

fn database(error: tokio_postgres::Error) -> RebornBuildError {
    RebornBuildError::OriginalShippedPure {
        source: Box::new(DatabaseCause(error)),
    }
}
fn check_transfer(length: usize) -> Result<(), RebornBuildError> {
    if length == 0 || wire::binary_data_row_bytes(&[Some(length), Some(64), Some(4)]).is_none() {
        return Err(invalid(
            "shipped R1 record response is not PG-representable",
        ));
    }
    Ok(())
}
async fn metadata(
    tx: &Transaction<'_>,
    id: Uuid,
) -> Result<Option<(usize, String)>, RebornBuildError> {
    tx.query_opt("SELECT octet_length(evidence_bytes),CASE WHEN octet_length(checksum)=64 THEN checksum END FROM reborn_component_review_evidence WHERE evidence_id=$1", &[&id])
        .await.map_err(database)?.map(|row| {
            let length=usize::try_from(row.get::<_, i32>(0)).map_err(invalid)?;
            check_transfer(length)?;
            let digest=row.get::<_,Option<String>>(1).filter(|value| value.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
                .ok_or_else(|| invalid("shipped R1 record digest is invalid"))?;
            Ok((length,digest))
        }).transpose()
}

/// Known exact PK, bounded original header, then guarded whole-record transfer.
/// No current settings participate. Envelope/subject authentication and the
/// complete native custody walk remain the caller's responsibility.
pub(super) async fn bootstrap(
    tx: &Transaction<'_>,
    id: Uuid,
    subject: &str,
    components: &[Value],
) -> Result<Option<(String, OriginalPolicy)>, RebornBuildError> {
    evidence::require_coherent(tx).await?;
    let Some((length, digest)) = metadata(tx, id).await? else {
        return Ok(None);
    };
    let marker = json!({"_frame":null});
    let view = evidence::system_record_view(id, "behavior", false, subject, components, &marker);
    const SUFFIX: &[u8] =
        b"null},\"reviewed_evidence\":[],\"succeeded\":false,\"validation_mode\":\"system_seed\"}";
    let count =
        usize::try_from(brassclaw_common::json::count_json_bytes(&view, None).map_err(invalid)?)
            .map_err(invalid)?;
    let start = count
        .checked_sub(SUFFIX.len())
        .ok_or_else(|| invalid("shipped R1 serializer prefix is invalid"))?;
    let maximum = start
        .checked_add(maximum_frame_bytes()?)
        .and_then(|size| size.checked_add(1))
        .ok_or_else(|| invalid("shipped R1 header size overflow"))?
        .min(length);
    if count > length || wire::binary_data_row_bytes(&[Some(maximum)]).is_none() {
        return Err(invalid("shipped R1 header exceeds PG capacity"));
    }
    let mut prefix = Vec::new();
    prefix.try_reserve_exact(count).map_err(invalid)?;
    brassclaw_common::json::write_json(&mut prefix, &view).map_err(invalid)?;
    if !prefix.ends_with(SUFFIX) {
        return Err(invalid("shipped R1 serializer framing differs"));
    }
    prefix.truncate(start);
    let maximum_sql = i32::try_from(maximum).map_err(invalid)?;
    // Limit characters before conversion as well: convert_to over the entire
    // TOASTed record would allocate its full body on the server just to read a
    // header. The outer byte slice still bounds the exact wire response.
    let row = tx.query_one("SELECT substring(convert_to(substring(evidence_bytes FROM 1 FOR $2),'UTF8') FROM 1 FOR $2) FROM reborn_component_review_evidence WHERE evidence_id=$1", &[&id,&maximum_sql])
        .await.map_err(database)?;
    let header: Vec<u8> = row.get(0);
    if header.len() > maximum || !header.starts_with(&prefix) {
        return Err(invalid("shipped R1 original header prefix differs"));
    }
    // Locate the first JSON value without copying a payload. This slice was
    // intrinsically bounded before transfer. The typed frame parser below then
    // rejects duplicate/unknown fields and noncanonical framing.
    let mut stream =
        serde_json::Deserializer::from_slice(&header[start..]).into_iter::<serde::de::IgnoredAny>();
    stream
        .next()
        .ok_or_else(|| invalid("shipped R1 frame is truncated"))?
        .map_err(invalid)?;
    let end = start
        .checked_add(stream.byte_offset())
        .ok_or_else(|| invalid("shipped R1 frame offset overflow"))?;
    let raw = std::str::from_utf8(&header[start..end]).map_err(invalid)?;
    let policy = OriginalPolicy::from_frame(raw)?;
    let bytes = policy
        .read(tx, id)
        .await?
        .ok_or_else(|| invalid("shipped R1 original disappeared"))?;
    if bytes.len() != length || evidence::checksum(&bytes) != digest {
        return Err(invalid("shipped R1 bootstrap metadata changed"));
    }
    Ok(Some((bytes, policy)))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn original(bytes: u64) -> RetainedPreparation {
        let policy = brassclaw_host_api::MontyExecutionLimits {
            max_value_bytes: bytes,
            ..Default::default()
        };
        RetainedPreparation::encode(
            17,
            brassclaw_host_api::task_preparation::PreparationOrigin::AcknowledgedStartup,
            policy,
        )
        .unwrap()
    }
    #[test]
    fn shipped_r1_frame_restores_original_and_rejects_duplicate_or_rewritten_policy() {
        let original = original(16 * 1024 * 1024);
        let policy = OriginalPolicy::new(&original).unwrap();
        let value = serde_json::to_value(&policy.frame).unwrap();
        let bytes = value.to_string();
        assert!(bytes.len() <= maximum_frame_bytes().unwrap());
        let restored = OriginalPolicy::from_frame(&bytes).unwrap();
        assert_eq!(restored.original().bytes(), original.bytes());
        assert_eq!(restored.limits().max_bytes, 16 * 1024 * 1024);
        for field in [
            "preparation_bytes",
            "preparation_checksum",
            "record_parser_limits",
        ] {
            let mut changed = value.clone();
            changed.as_object_mut().unwrap().remove(field);
            assert!(OriginalPolicy::from_frame(&changed.to_string()).is_err());
        }
        for changed in [
            format!("{{\"preparation_checksum\":\"bad\",{}", &bytes[1..]),
            {
                let mut v = value.clone();
                v["record_parser_limits"]["max_bytes"] = json!(32 * 1024 * 1024);
                v.to_string()
            },
            {
                let mut v = value.clone();
                v["unrecognized"] = json!(true);
                v.to_string()
            },
            {
                let mut v = value.clone();
                v["preparation_bytes"] = json!(format!(" {}", original.bytes()));
                v["preparation_checksum"] =
                    json!(evidence::checksum(v["preparation_bytes"].as_str().unwrap()));
                v.to_string()
            },
            format!(" {bytes}"),
        ] {
            assert!(OriginalPolicy::from_frame(&changed).is_err());
        }
        assert!(
            policy
                .encode(
                    Uuid::new_v4(),
                    "behavior",
                    false,
                    &"0".repeat(64),
                    &[],
                    json!({"_frame":{}})
                )
                .is_err()
        );
    }

    #[tokio::test]
    async fn native_shipped_r1_bootstrap_reads_original_policy_before_whole_body_and_keeps_pg_cause()
     {
        use std::error::Error;
        let rig = crate::runtime::test_pg::pg_rig().await;
        let _database = rig.lock_db().await;
        let policy = OriginalPolicy::new(&original(16 * 1024 * 1024)).unwrap();
        let id = Uuid::new_v4();
        let subject = evidence::checksum("closed framing test, no qualification");
        let report = json!({"format":"shipped-pure-r1-framing-test/1","data":"Ü🙂".repeat(5000)});
        let bytes = policy
            .encode(id, "behavior", false, &subject, &[], report.clone())
            .unwrap();
        let mut client = io::acquire(&rig.pool).await.unwrap();
        let tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .start()
            .await
            .unwrap();
        tx.execute(
            evidence::INTENT_INSERT,
            &[&id, &bytes, &evidence::checksum(&bytes)],
        )
        .await
        .unwrap();
        // Bootstrap is deliberately given neither original S nor current P.
        let (read, restored) = bootstrap(&tx, id, &subject, &[]).await.unwrap().unwrap();
        assert_eq!(read, bytes);
        assert_eq!(restored.original().bytes(), policy.original().bytes());
        assert_eq!(restored.limits().max_bytes, 16 * 1024 * 1024);
        assert!(
            bootstrap(&tx, id, &evidence::checksum("different subject"), &[])
                .await
                .is_err()
        );
        let small = OriginalPolicy::new(&original(1024)).unwrap();
        assert!(small.read(&tx, id).await.is_err());
        assert!(
            small
                .encode(id, "behavior", false, &subject, &[], report)
                .is_err()
        );
        tx.batch_execute("SELECT 1/0").await.unwrap_err();
        let error = policy.read(&tx, id).await.unwrap_err();
        assert!(!format!("{error:?}").contains("current transaction is aborted"));
        let mut source = error.source();
        let mut found = false;
        while let Some(error) = source {
            if let Some(pg) = error.downcast_ref::<tokio_postgres::Error>() {
                assert_eq!(pg.code().map(|code| code.code()), Some("25P02"));
                found = true;
            }
            source = error.source();
        }
        assert!(found);
        tx.rollback().await.unwrap();
        // No success or approval was manufactured; the test transaction and
        // its false-success framing row are both discarded.
    }
}
