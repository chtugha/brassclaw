//! Shared actual-owner contract. Parent supplies real encrypted stores/scope.
use super::{
    ResourceScope, SecretCustodySource, SecretCustodyState, SecretHandle, SecretStore,
    SecretStoreError,
};
use secrecy::ExposeSecret;
use std::sync::Arc;

async fn verify_identity_closure(
    first: Arc<dyn SecretStore>,
    second: Arc<dyn SecretStore>,
    scope: ResourceScope,
) {
    use super::SecretIdentityClosure as Closure;
    let handle = SecretHandle::new("custody-closed-before-late-writer").unwrap();
    let context = serde_json::json!({"original_flow":"closure-contract", "purpose":"pkce"});
    let expected = Closure::Absent {
        context: context.clone(),
    };
    let (a, b) = tokio::join!(
        first.close_identity(scope.clone(), handle.clone(), expected.clone()),
        second.close_identity(scope.clone(), handle.clone(), expected.clone()),
    );
    let closed = a.unwrap().unwrap();
    assert_eq!(
        b.unwrap(),
        Some(closed.clone()),
        "same closure preserves one actual record generation"
    );
    assert!(expected.accepts(&scope, &closed));
    assert!(matches!(
        closed.state,
        SecretCustodyState::ClosedIdentity { .. }
    ));
    assert!(first.lease_once(&scope, &handle).await.is_err());
    assert!(
        matches!(first.matches(&scope,&handle,&"late material".into()).await,
        Err(SecretStoreError::UnknownSecret {scope:actual_scope,handle:actual_handle})
            if *actual_scope==scope && actual_handle==handle),
        "closed identity must expose no comparable value, not a false live-value comparison"
    );
    assert!(
        second
            .put(scope.clone(), handle.clone(), "late material".into())
            .await
            .is_err()
    );
    assert!(
        second
            .put_original(
                scope.clone(),
                handle.clone(),
                "late material".into(),
                context.clone()
            )
            .await
            .unwrap()
            .is_none()
    );
    assert!(!second.delete(&scope, &handle).await.unwrap());
    assert!(
        second
            .close_identity(
                scope.clone(),
                handle.clone(),
                Closure::Absent {
                    context: serde_json::json!({"other":"flow"})
                }
            )
            .await
            .unwrap()
            .is_none()
    );
    let mut later = scope.clone();
    later.invocation_id = brassclaw_host_api::InvocationId::new();
    assert!(
        second
            .close_identity(later, handle.clone(), expected)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(first.custody(&scope, &handle).await.unwrap(), Some(closed));

    let original_handle = SecretHandle::new("custody-close-known-original").unwrap();
    let retained_handle = SecretHandle::new("custody-close-known-retained").unwrap();
    let original = first
        .put_original(
            scope.clone(),
            original_handle.clone(),
            "original raw material".into(),
            context.clone(),
        )
        .await
        .unwrap()
        .unwrap();
    let source = SecretCustodySource {
        scope: scope.clone(),
        handle: original_handle.clone(),
        reference: original.reference.clone(),
    };
    assert!(
        second
            .put_retained_from_original(scope.clone(), handle, source.clone())
            .await
            .unwrap()
            .is_none(),
        "closed absent name cannot become an alias"
    );
    let retained = second
        .put_retained_from_original(scope.clone(), retained_handle.clone(), source.clone())
        .await
        .unwrap()
        .unwrap();
    assert!(
        first
            .close_identity(
                scope.clone(),
                original_handle.clone(),
                Closure::Absent {
                    context: context.clone()
                }
            )
            .await
            .unwrap()
            .is_none(),
        "absence cannot retire an existing value"
    );
    let mut wrong = Closure::Original {
        reference: "unrelated-generation".into(),
        context: context.clone(),
    };
    assert!(
        first
            .close_identity(scope.clone(), original_handle.clone(), wrong.clone())
            .await
            .unwrap()
            .is_none()
    );
    wrong = Closure::Original {
        reference: original.reference.clone(),
        context: serde_json::json!({"other":"flow"}),
    };
    assert!(
        first
            .close_identity(scope.clone(), original_handle.clone(), wrong)
            .await
            .unwrap()
            .is_none()
    );
    let expected = Closure::Original {
        reference: original.reference.clone(),
        context,
    };
    let closed = first
        .close_identity(scope.clone(), original_handle.clone(), expected.clone())
        .await
        .unwrap()
        .unwrap();
    assert!(expected.accepts(&scope, &closed));
    assert_eq!(
        second
            .close_identity(scope.clone(), original_handle.clone(), expected)
            .await
            .unwrap(),
        Some(closed)
    );
    assert!(
        first
            .put_retained_from_original(
                scope.clone(),
                SecretHandle::new("late-custody-alias").unwrap(),
                source.clone()
            )
            .await
            .unwrap()
            .is_none()
    );
    let lease_before_closure = first.lease_once(&scope, &retained_handle).await.unwrap();
    let expected = Closure::Retained {
        reference: retained.reference,
        source,
    };
    let closed = second
        .close_identity(scope.clone(), retained_handle.clone(), expected.clone())
        .await
        .unwrap()
        .unwrap();
    assert!(expected.accepts(&scope, &closed));
    assert!(
        first
            .consume(&scope, lease_before_closure.id)
            .await
            .is_err(),
        "an already issued lease cannot consume retired material"
    );
    assert!(first.lease_once(&scope, &retained_handle).await.is_err());
    assert_eq!(
        first
            .close_identity(scope.clone(), retained_handle, expected)
            .await
            .unwrap(),
        Some(closed)
    );

    let ordinary = SecretHandle::new("custody-closure-cannot-delete-ordinary").unwrap();
    first
        .put(
            scope.clone(),
            ordinary.clone(),
            "ordinary live credential".into(),
        )
        .await
        .unwrap();
    assert!(
        second
            .close_identity(
                scope.clone(),
                ordinary.clone(),
                Closure::Absent {
                    context: serde_json::json!({})
                }
            )
            .await
            .unwrap()
            .is_none()
    );
    let lease = first.lease_once(&scope, &ordinary).await.unwrap();
    assert_eq!(
        first
            .consume(&scope, lease.id)
            .await
            .unwrap()
            .expose_secret(),
        "ordinary live credential"
    );
    let pending_lease = first.lease_once(&scope, &ordinary).await.unwrap();
    let reference = first
        .record_reference(&scope, &ordinary)
        .await
        .unwrap()
        .unwrap();
    let wrong = Closure::Ordinary {
        reference: "different ordinary generation".into(),
        context: serde_json::json!({"cleanup":"actual-account"}),
    };
    assert!(
        second
            .close_identity(scope.clone(), ordinary.clone(), wrong)
            .await
            .unwrap()
            .is_none()
    );
    let expected = Closure::Ordinary {
        reference: reference.clone(),
        context: serde_json::json!({"cleanup":"actual-account"}),
    };
    let closed = second
        .close_identity(scope.clone(), ordinary.clone(), expected.clone())
        .await
        .unwrap()
        .unwrap();
    assert!(
        matches!(&closed.state,SecretCustodyState::RetiredOrdinary {original_reference,..} if original_reference==&reference)
    );
    assert!(expected.accepts(&scope, &closed));
    assert!(first.consume(&scope, pending_lease.id).await.is_err());
    assert!(
        first
            .put(
                scope.clone(),
                ordinary.clone(),
                "late ordinary replacement".into()
            )
            .await
            .is_err()
    );
    assert_eq!(
        first
            .close_identity(scope, ordinary, expected)
            .await
            .unwrap(),
        Some(closed)
    );
}

pub(super) async fn verify(
    first: Arc<dyn SecretStore>,
    second: Arc<dyn SecretStore>,
    scope: ResourceScope,
) {
    verify_identity_closure(first.clone(), second.clone(), scope.clone()).await;
    let original_handle = SecretHandle::new("custody-contract-original").unwrap();
    let output_handle = SecretHandle::new("custody-contract-retained").unwrap();
    let second_output = SecretHandle::new("custody-contract-second-output").unwrap();
    let context = serde_json::json!({"flow":"actual-first-flow","interaction":"actual-first-interaction","selected_account":"original-selection"});
    let original = first
        .put_original(
            scope.clone(),
            original_handle.clone(),
            "exact original raw input".into(),
            context.clone(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        first.custody(&scope, &original_handle).await.unwrap(),
        Some(original.clone())
    );
    assert_eq!(
        first
            .record_reference(&scope, &original_handle)
            .await
            .unwrap(),
        Some(original.reference.clone())
    );
    assert_eq!(
        first
            .put_original(
                scope.clone(),
                original_handle.clone(),
                "exact original raw input".into(),
                context.clone()
            )
            .await
            .unwrap(),
        Some(original.clone())
    );
    assert!(
        first
            .put_original(
                scope.clone(),
                original_handle.clone(),
                "changed raw input".into(),
                context.clone()
            )
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        first
            .put_original(
                scope.clone(),
                original_handle.clone(),
                "exact original raw input".into(),
                serde_json::json!({"selected_account":"later-selection"})
            )
            .await
            .unwrap()
            .is_none()
    );
    let mut later_scope = scope.clone();
    later_scope.invocation_id = brassclaw_host_api::InvocationId::new();
    assert!(
        first
            .put_original(
                later_scope.clone(),
                original_handle.clone(),
                "exact original raw input".into(),
                context.clone()
            )
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        second
            .custody(&later_scope, &original_handle)
            .await
            .unwrap(),
        Some(original.clone()),
        "readback returns actual first scope, never latest caller projection"
    );
    assert!(
        first
            .metadata(&scope, &original_handle)
            .await
            .unwrap()
            .is_none()
    );
    assert!(first.lease_once(&scope, &original_handle).await.is_err());
    assert!(
        first
            .matches(&scope, &original_handle, &"exact original raw input".into())
            .await
            .unwrap()
    );
    assert!(
        !first
            .matches(&scope, &original_handle, &"changed raw input".into())
            .await
            .unwrap()
    );
    assert!(
        second
            .put(
                scope.clone(),
                original_handle.clone(),
                "normal overwrite".into()
            )
            .await
            .is_err()
    );
    assert!(
        second
            .put_if_record_reference(
                scope.clone(),
                original_handle.clone(),
                "conditional overwrite".into(),
                Some(&original.reference)
            )
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        first
            .record_reference(&scope, &original_handle)
            .await
            .unwrap(),
        Some(original.reference.clone())
    );
    let source = SecretCustodySource {
        scope: original.scope.clone(),
        handle: original.handle.clone(),
        reference: original.reference.clone(),
    };
    let mut foreign_scope = scope.clone();
    foreign_scope.user_id = brassclaw_host_api::UserId::new("foreign-custody-owner").unwrap();
    assert!(
        second
            .custody(&foreign_scope, &original_handle)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        second
            .lease_once(&foreign_scope, &original_handle)
            .await
            .is_err()
    );
    assert!(
        second
            .put_if_record_reference(
                foreign_scope.clone(),
                original_handle.clone(),
                "foreign replacement".into(),
                Some(&original.reference)
            )
            .await
            .unwrap()
            .is_none()
    );
    let mut foreign_source = source.clone();
    foreign_source.scope = foreign_scope;
    assert!(
        second
            .put_retained_from_original(scope.clone(), output_handle.clone(), foreign_source)
            .await
            .is_err()
    );
    assert_eq!(
        first.custody(&scope, &original_handle).await.unwrap(),
        Some(original.clone())
    );
    let mut bad_source = source.clone();
    bad_source.reference = "unrelated encrypted generation".into();
    assert!(
        second
            .put_retained_from_original(scope.clone(), output_handle.clone(), bad_source)
            .await
            .unwrap()
            .is_none()
    );
    let mut bad_source = source.clone();
    bad_source.scope.invocation_id = brassclaw_host_api::InvocationId::new();
    assert!(
        second
            .put_retained_from_original(scope.clone(), output_handle.clone(), bad_source)
            .await
            .unwrap()
            .is_none()
    );
    let output = second
        .put_retained_from_original(scope.clone(), output_handle.clone(), source.clone())
        .await
        .unwrap()
        .unwrap();
    assert!(
        matches!(&output.state,SecretCustodyState::Retained { source: actual } if actual==&source)
    );
    assert_eq!(
        first
            .put_retained_from_original(scope.clone(), output_handle.clone(), source.clone())
            .await
            .unwrap(),
        Some(output.clone())
    );
    assert_eq!(
        first.custody(&scope, &output_handle).await.unwrap(),
        Some(output.clone())
    );
    assert!(
        first
            .metadata(&scope, &output_handle)
            .await
            .unwrap()
            .is_some()
    );
    let lease = first.lease_once(&scope, &output_handle).await.unwrap();
    assert_eq!(
        first
            .consume(&scope, lease.id)
            .await
            .unwrap()
            .expose_secret(),
        "exact original raw input"
    );
    let consumed = first.consume(&scope, lease.id).await.unwrap_err();
    // Native PG removes the consumed lease atomically; memory/FS retain its
    // terminal status. Both existing contracts reject a second raw handoff.
    assert!(consumed.is_consumed() || consumed.is_unknown_lease());
    assert!(
        first
            .put(
                scope.clone(),
                output_handle.clone(),
                "normal output overwrite".into()
            )
            .await
            .is_err()
    );
    assert!(
        first
            .put_if_record_reference(
                scope.clone(),
                output_handle.clone(),
                "output CAS overwrite".into(),
                Some(&output.reference)
            )
            .await
            .unwrap()
            .is_none()
    );
    let waiting = first.lease_once(&scope, &output_handle).await.unwrap();
    assert!(second.delete(&scope, &output_handle).await.unwrap());
    let retired = first
        .custody(&scope, &output_handle)
        .await
        .unwrap()
        .unwrap();
    assert!(
        matches!(&retired.state,SecretCustodyState::RetiredRetained { source: actual,retained_reference } if actual==&source && retained_reference==&output.reference)
    );
    assert_ne!(retired.reference, output.reference);
    assert!(
        first.consume(&scope, waiting.id).await.is_err(),
        "retirement never releases old leased material"
    );
    assert!(first.lease_once(&scope, &output_handle).await.is_err());
    assert!(
        first
            .metadata(&scope, &output_handle)
            .await
            .unwrap()
            .is_none()
    );
    assert!(!second.delete(&scope, &output_handle).await.unwrap());
    assert!(
        first
            .put(scope.clone(), output_handle.clone(), "resurrection".into())
            .await
            .is_err()
    );
    assert!(
        first
            .put_if_record_reference(
                scope.clone(),
                output_handle.clone(),
                "retired CAS overwrite".into(),
                Some(&retired.reference)
            )
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        first
            .put_original(
                scope.clone(),
                output_handle.clone(),
                "resurrection".into(),
                context.clone()
            )
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        first
            .put_retained_from_original(scope.clone(), output_handle.clone(), source.clone())
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        first.custody(&scope, &output_handle).await.unwrap(),
        Some(retired)
    );
    let retained_before_origin_disposition = first
        .put_retained_from_original(scope.clone(), second_output.clone(), source.clone())
        .await
        .unwrap()
        .unwrap();
    assert!(second.delete(&scope, &original_handle).await.unwrap());
    let retired_original = first
        .custody(&scope, &original_handle)
        .await
        .unwrap()
        .unwrap();
    assert!(
        matches!(&retired_original.state,SecretCustodyState::RetiredOriginal { original_reference,context: actual } if original_reference==&source.reference && actual==&context)
    );
    assert!(
        first
            .put_original(
                scope.clone(),
                original_handle.clone(),
                "exact original raw input".into(),
                context
            )
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        first
            .put_retained_from_original(
                scope.clone(),
                SecretHandle::new("after-origin-retirement").unwrap(),
                source
            )
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        first
            .matches(&scope, &original_handle, &"exact original raw input".into())
            .await
            .is_err()
    );
    assert!(first.lease_once(&scope, &original_handle).await.is_err());
    // Original input retirement is data disposition, not revocation of an
    // independently issued output or a bypass of live kernel/account policy.
    assert_eq!(
        first.custody(&scope, &second_output).await.unwrap(),
        Some(retained_before_origin_disposition)
    );
    let still_selected = first.lease_once(&scope, &second_output).await.unwrap();
    assert_eq!(
        first
            .consume(&scope, still_selected.id)
            .await
            .unwrap()
            .expose_secret(),
        "exact original raw input"
    );

    let race = SecretHandle::new("custody-contract-race").unwrap();
    let context = serde_json::json!({"first":"racing-input"});
    let (a, b) = tokio::join!(
        first.put_original(
            scope.clone(),
            race.clone(),
            "candidate-alpha".into(),
            context.clone()
        ),
        second.put_original(
            scope.clone(),
            race.clone(),
            "candidate-beta".into(),
            context.clone()
        )
    );
    let (winner, value) = match (a.unwrap(), b.unwrap()) {
        (Some(meta), None) => (meta, "candidate-alpha"),
        (None, Some(meta)) => (meta, "candidate-beta"),
        other => panic!("one exact original winner required: {other:?}"),
    };
    assert!(first.matches(&scope, &race, &value.into()).await.unwrap());
    assert_eq!(second.custody(&scope, &race).await.unwrap(), Some(winner));
    let equal = SecretHandle::new("custody-contract-equal-race").unwrap();
    let (a, b) = tokio::join!(
        first.put_original(
            scope.clone(),
            equal.clone(),
            "identical".into(),
            context.clone()
        ),
        second.put_original(scope.clone(), equal.clone(), "identical".into(), context)
    );
    assert_eq!(
        a.unwrap().unwrap(),
        b.unwrap().unwrap(),
        "same-input insert loser reads exact actual winner"
    );
    let competing = SecretHandle::new("custody-contract-normal-race").unwrap();
    let (a, b) = tokio::join!(
        first.put_original(
            scope.clone(),
            competing.clone(),
            "original-winner".into(),
            serde_json::json!({"race":"normal-writer"})
        ),
        second.put(scope.clone(), competing.clone(), "ordinary-winner".into())
    );
    match a.unwrap() {
        Some(meta) => {
            assert!(b.is_err());
            assert_eq!(first.custody(&scope, &competing).await.unwrap(), Some(meta));
            assert!(
                first
                    .matches(&scope, &competing, &"original-winner".into())
                    .await
                    .unwrap()
            );
        }
        None => {
            b.unwrap();
            assert!(first.custody(&scope, &competing).await.unwrap().is_none());
            assert!(
                first
                    .matches(&scope, &competing, &"ordinary-winner".into())
                    .await
                    .unwrap()
            );
        }
    }
    let ordinary = SecretHandle::new("custody-looking-ordinary").unwrap();
    let raw = r#"{"kind":"original","material":"still ordinary raw JSON"}"#;
    first
        .put(scope.clone(), ordinary.clone(), raw.into())
        .await
        .unwrap();
    assert!(first.custody(&scope, &ordinary).await.unwrap().is_none());
    let lease = first.lease_once(&scope, &ordinary).await.unwrap();
    assert_eq!(
        first
            .consume(&scope, lease.id)
            .await
            .unwrap()
            .expose_secret(),
        raw
    );
}
