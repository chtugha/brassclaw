use super::*;
use brassclaw_filesystem::InMemoryBackend;
use brassclaw_host_api::{
    AgentId, MountAlias, MountGrant, MountPermissions, MountView, TenantId, VirtualPath,
};

async fn fixture() -> (
    FilesystemSessionThreadService<InMemoryBackend>,
    AcceptInboundMessageRequest,
) {
    let mounts = MountView::new(vec![MountGrant::new(
        MountAlias::new("/threads").unwrap(),
        VirtualPath::new("/tenants/sequence-owner/users/sequence-user/threads").unwrap(),
        MountPermissions::read_write_list_delete(),
    )])
    .unwrap();
    let filesystem = Arc::new(ScopedFilesystem::with_fixed_view(
        Arc::new(InMemoryBackend::new()),
        mounts,
    ));
    let service = FilesystemSessionThreadService::new(filesystem);
    let scope = ThreadScope {
        tenant_id: TenantId::new("sequence-owner").unwrap(),
        agent_id: AgentId::new("sequence-agent").unwrap(),
        project_id: None,
        owner_user_id: None,
    };
    let thread = service
        .ensure_thread(EnsureThreadRequest {
            scope: scope.clone(),
            thread_id: None,
            created_by_actor_id: "owner".into(),
            title: None,
            metadata_json: None,
        })
        .await
        .unwrap();
    (
        service,
        AcceptInboundMessageRequest {
            scope,
            thread_id: thread.thread_id,
            actor_id: "owner".into(),
            source_binding_id: Some("source".into()),
            reply_target_binding_id: None,
            external_event_id: Some("event".into()),
            content: MessageContent::text("preserve input"),
        },
    )
}

#[tokio::test]
async fn actual_filesystem_pending_receipt_cannot_overwrite_or_create_another_message() {
    let (service, request) = fixture().await;
    let key = InboundIdempotencyKey::from_request(&request).unwrap();
    let path = idempotency_record_path(&idempotency_record_key(&key).unwrap()).unwrap();
    let original = InboundIdempotencyRecord {
        scope: request.scope.clone(),
        thread_id: request.thread_id.clone(),
        message_id: ThreadMessageId::new(),
        source_binding_id: key.source_binding_id,
        external_event_id: key.external_event_id,
    };
    service
        .filesystem
        .put(
            &request.scope.to_resource_scope(),
            &path,
            FilesystemSessionThreadService::<InMemoryBackend>::idempotency_entry(&original)
                .unwrap(),
            CasExpectation::Absent,
        )
        .await
        .unwrap();
    let before = service
        .filesystem
        .get(&request.scope.to_resource_scope(), &path)
        .await
        .unwrap()
        .unwrap();
    assert!(
        matches!(service.accept_inbound_message(request.clone()).await,
        Err(SessionThreadError::UnknownMessage {message_id}) if message_id==original.message_id)
    );
    let after = service
        .filesystem
        .get(&request.scope.to_resource_scope(), &path)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(before.version, after.version);
    assert_eq!(before.entry.body, after.entry.body);
    assert!(
        service
            .list_thread_history(ThreadHistoryRequest {
                scope: request.scope,
                thread_id: request.thread_id
            })
            .await
            .unwrap()
            .messages
            .is_empty()
    );
}

#[tokio::test]
async fn exhausted_filesystem_sequence_keeps_event_unclaimed_and_message_absent() {
    let (service, request) = fixture().await;
    let (mut record, version) = service
        .read_thread_versioned(&request.scope, &request.thread_id)
        .await
        .unwrap()
        .unwrap();
    record.next_sequence = u64::MAX;
    let path = thread_record_path(&request.scope, &request.thread_id).unwrap();
    service
        .filesystem
        .put(
            &request.scope.to_resource_scope(),
            &path,
            FilesystemSessionThreadService::<InMemoryBackend>::thread_entry(&record).unwrap(),
            CasExpectation::Version(version),
        )
        .await
        .unwrap();
    let before = service
        .filesystem
        .get(&request.scope.to_resource_scope(), &path)
        .await
        .unwrap()
        .unwrap();
    assert!(
        matches!(service.accept_inbound_message(request.clone()).await,
        Err(SessionThreadError::Backend(reason)) if reason=="thread message sequence exhausted")
    );
    let after = service
        .filesystem
        .get(&request.scope.to_resource_scope(), &path)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(before.version, after.version);
    assert_eq!(before.entry.body, after.entry.body);
    let key = InboundIdempotencyKey::from_request(&request).unwrap();
    let receipt = idempotency_record_path(&idempotency_record_key(&key).unwrap()).unwrap();
    assert!(
        service
            .filesystem
            .get(&request.scope.to_resource_scope(), &receipt)
            .await
            .unwrap()
            .is_none()
    );
}
