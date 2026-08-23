#[cfg(feature = "postgres")]
use brassclaw_events::{EventCursor, EventStreamKey, ReadScope, RuntimeEvent};
#[cfg(feature = "postgres")]
use brassclaw_host_api::{
    ActionResultSummary, ActionSummary, AgentId, AuditEnvelope, AuditEventId, AuditStage,
    CapabilityId, CorrelationId, DecisionSummary, ExtensionId, InvocationId, ProjectId,
    ResourceScope, RuntimeKind, TenantId, UserId,
};
#[cfg(feature = "postgres")]
use brassclaw_reborn_event_store::{
    RebornEventStoreConfig, RebornProfile, build_reborn_event_stores,
};
#[cfg(feature = "postgres")]
use secrecy::SecretString;

#[cfg(feature = "postgres")]
fn capability_id() -> CapabilityId {
    CapabilityId::new("demo.echo").expect("capability id")
}

#[cfg(feature = "postgres")]
fn extension_id() -> ExtensionId {
    ExtensionId::new("demo").expect("extension id")
}

#[cfg(feature = "postgres")]
fn scope_for(user: &str, project: &str) -> ResourceScope {
    ResourceScope {
        tenant_id: TenantId::new("default").expect("tenant id"),
        user_id: UserId::new(user).expect("user id"),
        agent_id: Some(AgentId::new("default").expect("agent id")),
        project_id: Some(ProjectId::new(project).expect("project id")),
        thread_id: None,
        invocation_id: InvocationId::new(),
    }
}

#[cfg(feature = "postgres")]
fn audit_record(scope: &ResourceScope, status: &str) -> AuditEnvelope {
    AuditEnvelope {
        event_id: AuditEventId::new(),
        correlation_id: CorrelationId::new(),
        stage: AuditStage::After,
        timestamp: chrono::Utc::now(),
        tenant_id: scope.tenant_id.clone(),
        user_id: scope.user_id.clone(),
        agent_id: scope.agent_id.clone(),
        project_id: scope.project_id.clone(),
        thread_id: scope.thread_id.clone(),
        invocation_id: scope.invocation_id,
        process_id: None,
        approval_request_id: None,
        extension_id: Some(extension_id()),
        action: ActionSummary {
            kind: "dispatch".to_string(),
            target: Some(capability_id().as_str().to_string()),
            effects: Vec::new(),
        },
        decision: DecisionSummary {
            kind: "allow".to_string(),
            reason: None,
            actor: None,
        },
        result: Some(ActionResultSummary {
            success: true,
            status: Some(status.to_string()),
            output_bytes: Some(12),
        }),
    }
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_replay_advances_next_cursor_past_trailing_filtered_records() {
    let Ok(url) = std::env::var("BRASSCLAW_REBORN_EVENT_STORE_POSTGRES_URL") else {
        eprintln!(
            "skipping postgres event-store cursor contract: BRASSCLAW_REBORN_EVENT_STORE_POSTGRES_URL not set"
        );
        return;
    };
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let scope_a = scope_for(&format!("postgres-tail-alice-{suffix}"), "project-a");
    let scope_b = scope_for(&format!("postgres-tail-alice-{suffix}"), "project-b");
    let stream = EventStreamKey::from_scope(&scope_a);

    let stores = build_reborn_event_stores(
        RebornProfile::Production,
        RebornEventStoreConfig::Postgres {
            url: SecretString::new(url.into_boxed_str()),
        },
    )
    .await
    .expect("postgres stores");

    stores
        .events
        .append(RuntimeEvent::dispatch_requested(
            scope_a.clone(),
            capability_id(),
        ))
        .await
        .expect("append project a");
    stores
        .events
        .append(RuntimeEvent::dispatch_requested(
            scope_b.clone(),
            capability_id(),
        ))
        .await
        .expect("append trailing project b");

    let project_a = ReadScope {
        project_id: scope_a.project_id.clone(),
        ..ReadScope::default()
    };
    let replay = stores
        .events
        .read_after_cursor(&stream, &project_a, None, 10)
        .await
        .expect("replay project a");

    assert_eq!(replay.entries.len(), 1);
    assert_eq!(replay.entries[0].cursor, EventCursor::new(1));
    assert_eq!(
        replay.next_cursor,
        EventCursor::new(2),
        "filtered trailing records must advance Postgres replay cursor"
    );
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_runtime_and_audit_logs_survive_rebuild_with_filtered_cursor_semantics() {
    let Ok(url) = std::env::var("BRASSCLAW_REBORN_EVENT_STORE_POSTGRES_URL") else {
        eprintln!(
            "skipping postgres event-store contract: BRASSCLAW_REBORN_EVENT_STORE_POSTGRES_URL not set"
        );
        return;
    };
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let scope_a = scope_for(&format!("postgres-alice-{suffix}"), "project-a");
    let scope_b = scope_for(&format!("postgres-alice-{suffix}"), "project-b");
    let stream = EventStreamKey::from_scope(&scope_a);

    let stores = build_reborn_event_stores(
        RebornProfile::Production,
        RebornEventStoreConfig::Postgres {
            url: SecretString::new(url.clone().into_boxed_str()),
        },
    )
    .await
    .expect("postgres stores");

    stores
        .events
        .append(RuntimeEvent::dispatch_requested(
            scope_a.clone(),
            capability_id(),
        ))
        .await
        .expect("append project a 1");
    stores
        .events
        .append(RuntimeEvent::dispatch_requested(
            scope_b.clone(),
            capability_id(),
        ))
        .await
        .expect("append project b");
    stores
        .events
        .append(RuntimeEvent::dispatch_succeeded(
            scope_a.clone(),
            capability_id(),
            extension_id(),
            RuntimeKind::Mcp,
            7,
        ))
        .await
        .expect("append project a 2");
    stores
        .audit
        .append(audit_record(&scope_a, "project-a"))
        .await
        .expect("append project a audit");
    stores
        .audit
        .append(audit_record(&scope_b, "project-b"))
        .await
        .expect("append project b audit");
    drop(stores);

    let stores = build_reborn_event_stores(
        RebornProfile::Production,
        RebornEventStoreConfig::Postgres {
            url: SecretString::new(url.into_boxed_str()),
        },
    )
    .await
    .expect("postgres stores after reconnect");

    let project_a = ReadScope {
        project_id: scope_a.project_id.clone(),
        ..ReadScope::default()
    };
    let replay = stores
        .events
        .read_after_cursor(&stream, &project_a, None, 10)
        .await
        .expect("runtime replay");
    assert_eq!(replay.entries.len(), 2);
    assert_eq!(replay.entries[0].cursor, EventCursor::new(1));
    assert_eq!(replay.entries[1].cursor, EventCursor::new(3));

    let audit_replay = stores
        .audit
        .read_after_cursor(&stream, &project_a, None, 10)
        .await
        .expect("audit replay");
    assert_eq!(audit_replay.entries.len(), 1);
    assert_eq!(
        audit_replay.entries[0]
            .record
            .result
            .as_ref()
            .unwrap()
            .status,
        Some("project-a".to_string())
    );
}
