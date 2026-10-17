//! Global UUID-keyed settings over the same durable policy rows used by Monty's
//! kernel. No tenant grants, alias guessing, component activation or new rows.
use async_trait::async_trait;
use brassclaw_pg::PgPool;
use brassclaw_product_workflow::instance_tools::{
    InstanceToolSetting, InstanceToolSettings, InstanceToolSettingsError, UpdateInstanceToolSetting,
};
use std::sync::Arc;
use tokio_postgres::Row;
use uuid::Uuid;

pub(crate) struct PgInstanceToolSettings {
    pool: Arc<PgPool>,
}
impl PgInstanceToolSettings {
    pub(crate) fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

// One statement reads each policy and its complete retained alias set. These
// aliases describe declared records; this is not registration/approval evidence.
// An unfinished draft can have no alias. That must not prevent an operator from
// blocking its UUID or overwrite a running task's independently pinned identity.
const READ: &str = "SELECT s.tool_id,s.enabled,s.revision,h.class_code,
    array_agg(DISTINCT r.revision_bytes::jsonb->'document'->>'capability_id'
        ORDER BY r.revision_bytes::jsonb->'document'->>'capability_id')
        FILTER (WHERE jsonb_typeof(r.revision_bytes::jsonb->'document'->'capability_id')='string')
    FROM brassclaw_instance_tool_settings s
    JOIN reborn_component_revision_heads h ON h.component_id=s.tool_id
    LEFT JOIN reborn_component_revisions r ON r.component_id=s.tool_id
    WHERE ($1::uuid IS NULL OR s.tool_id=$1)
    GROUP BY s.tool_id,s.enabled,s.revision,h.class_code ORDER BY s.tool_id";
fn row(value: &Row) -> Result<InstanceToolSetting, InstanceToolSettingsError> {
    let tool_id: Uuid = value.get(0);
    let revision = u64::try_from(value.get::<_, i64>(2))
        .map_err(|_| InstanceToolSettingsError::Unavailable)?;
    let aliases = value.get::<_, Option<Vec<String>>>(4).unwrap_or_default();
    if tool_id.is_nil() || revision == 0 || value.get::<_, i16>(3) != 0 {
        return Err(InstanceToolSettingsError::Unavailable);
    }
    Ok(InstanceToolSetting {
        tool_id,
        enabled: value.get(1),
        revision,
        capability_ids: aliases,
    })
}
#[async_trait]
impl InstanceToolSettings for PgInstanceToolSettings {
    async fn list(&self) -> Result<Vec<InstanceToolSetting>, InstanceToolSettingsError> {
        let client = self
            .pool
            .get()
            .await
            .map_err(|_| InstanceToolSettingsError::Unavailable)?;
        client
            .query(READ, &[&None::<Uuid>])
            .await
            .map_err(|_| InstanceToolSettingsError::Unavailable)?
            .iter()
            .map(row)
            .collect()
    }
    async fn update(
        &self,
        tool_id: Uuid,
        request: &UpdateInstanceToolSetting,
    ) -> Result<InstanceToolSetting, InstanceToolSettingsError> {
        let expected = i64::try_from(request.expected_revision)
            .map_err(|_| InstanceToolSettingsError::Invalid)?;
        if tool_id.is_nil() || expected <= 0 || expected == i64::MAX {
            return Err(InstanceToolSettingsError::Invalid);
        }
        let mut client = self
            .pool
            .get()
            .await
            .map_err(|_| InstanceToolSettingsError::Unavailable)?;
        let tx = client
            .transaction()
            .await
            .map_err(|_| InstanceToolSettingsError::Unavailable)?;
        // Check identity/retained metadata before mutation; a corrupt policy is
        // an error to repair, never silently excluded or turned into a new grant.
        let current = tx
            .query_opt(READ, &[&Some(tool_id)])
            .await
            .map_err(|_| InstanceToolSettingsError::Unavailable)?
            .ok_or(InstanceToolSettingsError::NotFound)?;
        let current = row(&current)?;
        if current.revision != request.expected_revision {
            return Err(InstanceToolSettingsError::Conflict);
        }
        let changed = tx
            .execute(
                "UPDATE brassclaw_instance_tool_settings SET enabled=$2,revision=revision+1
            WHERE tool_id=$1 AND revision=$3",
                &[&tool_id, &request.enabled, &expected],
            )
            .await
            .map_err(|_| InstanceToolSettingsError::Unavailable)?;
        if changed != 1 {
            return Err(InstanceToolSettingsError::Conflict);
        }
        // Decode before commit; failure cannot publish a durable value while
        // returning an error from conversion. A lost response can be refetched;
        // stale retries never perform a second change under the same revision.
        let updated = tx
            .query_one(READ, &[&Some(tool_id)])
            .await
            .map_err(|_| InstanceToolSettingsError::Unavailable)?;
        let updated = row(&updated)?;
        tx.commit()
            .await
            .map_err(|_| InstanceToolSettingsError::Unavailable)?;
        Ok(updated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::test_pg::native_pg;
    use brassclaw_skills::{
        component_revision::ComponentRevisionDraft, revision_store::PgComponentRevisionStore,
    };
    use serde_json::json;

    #[tokio::test]
    async fn native_instance_tool_settings_keep_cas_aliases_and_rejection_atomic() {
        let rig = native_pg::NativePostgres::start().await;
        let revisions = PgComponentRevisionStore::new(rig.pool.clone());
        let tool = Uuid::new_v4();
        // Real unapproved retained metadata. Neither this fixture nor policy
        // management registers, approves or activates an executable component.
        let draft = |alias: &str| {
            ComponentRevisionDraft::from_json(
                &json!({
                    "format":"component-revision/1","uuid":tool,"class_code":0,
                    "document":{"capability_id":alias},"dependencies":[],"association":null
                })
                .to_string(),
            )
            .unwrap()
        };
        revisions
            .stage(&draft("fixture.old_alias"), 0)
            .await
            .unwrap();
        let client = rig.pool.get().await.unwrap();
        client
            .execute(
                "INSERT INTO brassclaw_instance_tool_settings(tool_id) VALUES($1)",
                &[&tool],
            )
            .await
            .unwrap();
        let settings = Arc::new(PgInstanceToolSettings::new(rig.pool.clone()));
        assert_eq!(
            settings.list().await.unwrap()[0].capability_ids,
            ["fixture.old_alias"]
        );
        revisions
            .stage(&draft("fixture.new_alias"), 1)
            .await
            .unwrap();
        let listed = settings.list().await.unwrap();
        assert_eq!(
            listed[0].capability_ids,
            ["fixture.new_alias", "fixture.old_alias"]
        );
        let request = UpdateInstanceToolSetting {
            expected_revision: 1,
            enabled: false,
        };
        let (first, second) = tokio::join!(
            settings.update(tool, &request),
            settings.update(tool, &request)
        );
        assert!(matches!(
            (&first, &second),
            (Ok(_), Err(InstanceToolSettingsError::Conflict))
                | (Err(InstanceToolSettingsError::Conflict), Ok(_))
        ));
        let before = settings.list().await.unwrap();
        assert_eq!(before[0].revision, 2);
        assert!(!before[0].enabled);
        for expected in [0, u64::MAX, i64::MAX as u64] {
            assert!(matches!(
                settings
                    .update(
                        tool,
                        &UpdateInstanceToolSetting {
                            expected_revision: expected,
                            enabled: true
                        }
                    )
                    .await,
                Err(InstanceToolSettingsError::Invalid)
            ));
        }
        assert!(matches!(
            settings.update(Uuid::new_v4(), &request).await,
            Err(InstanceToolSettingsError::NotFound)
        ));
        assert!(matches!(
            settings.update(Uuid::nil(), &request).await,
            Err(InstanceToolSettingsError::Invalid)
        ));
        assert!(matches!(
            settings.update(tool, &request).await,
            Err(InstanceToolSettingsError::Conflict)
        ));
        assert_eq!(settings.list().await.unwrap(), before);
        client.batch_execute("CREATE FUNCTION reject_instance_tool_edit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'deliberate policy rejection' USING ERRCODE='23514'; END $$; CREATE TRIGGER reject_instance_tool_edit BEFORE UPDATE ON brassclaw_instance_tool_settings FOR EACH ROW EXECUTE FUNCTION reject_instance_tool_edit()").await.unwrap();
        let next = UpdateInstanceToolSetting {
            expected_revision: 2,
            enabled: true,
        };
        assert!(matches!(
            settings.update(tool, &next).await,
            Err(InstanceToolSettingsError::Unavailable)
        ));
        assert_eq!(settings.list().await.unwrap(), before);
        client.batch_execute("DROP TRIGGER reject_instance_tool_edit ON brassclaw_instance_tool_settings; DROP FUNCTION reject_instance_tool_edit()").await.unwrap();
        let updated = settings.update(tool, &next).await.unwrap();
        assert_eq!(updated.revision, 3);
        assert!(updated.enabled);
        assert_eq!(updated.capability_ids, before[0].capability_ids);
        let wrong = Uuid::new_v4();
        let wrong_draft = ComponentRevisionDraft::from_json(&json!({"format":"component-revision/1","uuid":wrong,"class_code":13,"document":{},"dependencies":[],"association":null}).to_string()).unwrap();
        revisions.stage(&wrong_draft, 0).await.unwrap();
        let rejection = client
            .execute(
                "INSERT INTO brassclaw_instance_tool_settings(tool_id) VALUES($1)",
                &[&wrong],
            )
            .await
            .unwrap_err();
        assert_eq!(
            rejection.code(),
            Some(&tokio_postgres::error::SqlState::FOREIGN_KEY_VIOLATION)
        );
        assert!(matches!(
            settings.update(wrong, &request).await,
            Err(InstanceToolSettingsError::NotFound)
        ));
        assert_eq!(settings.list().await.unwrap(), [updated]);
        assert_eq!(
            client
                .query_one(
                    "SELECT count(*) FROM reborn_skill_association_approvals",
                    &[]
                )
                .await
                .unwrap()
                .get::<_, i64>(0),
            0
        );
    }
}
