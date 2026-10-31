//! Shared native contract; parent owns the actual stores, scope and database.
use super::{ResourceScope, SecretHandle, SecretStore};
use secrecy::ExposeSecret;
use std::sync::Arc;

pub(super) async fn verify_reference_contract(
    first: Arc<dyn SecretStore>,
    second: Arc<dyn SecretStore>,
    scope: ResourceScope,
) {
    let handle = SecretHandle::new("generation-reference-probe").unwrap();
    assert!(
        first
            .record_reference(&scope, &handle)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        first
            .matches(&scope, &handle, &"absent".into())
            .await
            .unwrap_err()
            .is_unknown_secret()
    );
    let (a, b) = tokio::join!(
        first.put_if_record_reference(
            scope.clone(),
            handle.clone(),
            "candidate-alpha".into(),
            None
        ),
        second.put_if_record_reference(
            scope.clone(),
            handle.clone(),
            "candidate-beta".into(),
            None
        ),
    );
    let winner = match (a.unwrap(), b.unwrap()) {
        (Some(_), None) => "candidate-alpha",
        (None, Some(_)) => "candidate-beta",
        other => panic!("one actual absent-only winner required: {other:?}"),
    };
    let original = first
        .record_reference(&scope, &handle)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        second.record_reference(&scope, &handle).await.unwrap(),
        Some(original.clone())
    );
    assert!(
        first
            .matches(&scope, &handle, &winner.into())
            .await
            .unwrap()
    );
    assert!(
        !first
            .matches(&scope, &handle, &"different-value".into())
            .await
            .unwrap()
    );
    assert_eq!(
        second.record_reference(&scope, &handle).await.unwrap(),
        Some(original.clone())
    );
    assert!(
        second
            .put_if_record_reference(scope.clone(), handle.clone(), "changed".into(), None)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        second
            .put_if_record_reference(
                scope.clone(),
                handle.clone(),
                "changed".into(),
                Some("unrelated-reference")
            )
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        first.record_reference(&scope, &handle).await.unwrap(),
        Some(original.clone())
    );

    let mut foreign = scope.clone();
    foreign.user_id = brassclaw_host_api::UserId::new("foreign-reference-owner").unwrap();
    assert!(
        first
            .record_reference(&foreign, &handle)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        first
            .matches(&foreign, &handle, &winner.into())
            .await
            .unwrap_err()
            .is_unknown_secret()
    );
    assert!(
        first
            .put_if_record_reference(
                foreign,
                handle.clone(),
                "foreign-value".into(),
                Some(&original)
            )
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        first
            .matches(&scope, &handle, &winner.into())
            .await
            .unwrap()
    );

    // A normal concurrent writer changes the authenticated generation. Snapshot
    // CAS must never overwrite that later value, even if the old value is known.
    second
        .put(scope.clone(), handle.clone(), "operator-replacement".into())
        .await
        .unwrap();
    let replacement = second
        .record_reference(&scope, &handle)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(replacement, original);
    assert!(
        first
            .put_if_record_reference(
                scope.clone(),
                handle.clone(),
                "stale-value".into(),
                Some(&original)
            )
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        first
            .matches(&scope, &handle, &"operator-replacement".into())
            .await
            .unwrap()
    );
    let (a, b) = tokio::join!(
        first.put_if_record_reference(
            scope.clone(),
            handle.clone(),
            "successor-alpha".into(),
            Some(&replacement)
        ),
        second.put_if_record_reference(
            scope.clone(),
            handle.clone(),
            "successor-beta".into(),
            Some(&replacement)
        ),
    );
    let successor = match (a.unwrap(), b.unwrap()) {
        (Some(_), None) => "successor-alpha",
        (None, Some(_)) => "successor-beta",
        other => panic!("one captured-generation successor required: {other:?}"),
    };
    assert!(
        second
            .matches(&scope, &handle, &successor.into())
            .await
            .unwrap()
    );
    let before_delete = second
        .record_reference(&scope, &handle)
        .await
        .unwrap()
        .unwrap();
    assert!(first.delete(&scope, &handle).await.unwrap());
    assert!(
        second
            .put_if_record_reference(
                scope.clone(),
                handle.clone(),
                successor.into(),
                Some(&before_delete)
            )
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        second
            .put_if_record_reference(scope.clone(), handle.clone(), successor.into(), None)
            .await
            .unwrap()
            .is_some()
    );
    let after_recreate = first
        .record_reference(&scope, &handle)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(
        after_recreate, before_delete,
        "same material is not the deleted generation"
    );
    assert!(
        first
            .put_if_record_reference(
                scope.clone(),
                handle.clone(),
                "stale-after-recreate".into(),
                Some(&before_delete)
            )
            .await
            .unwrap()
            .is_none()
    );

    // Fresh leases keep their actual one-shot and owner checks.
    let lease = first.lease_once(&scope, &handle).await.unwrap();
    let mut foreign = scope.clone();
    foreign.user_id = brassclaw_host_api::UserId::new("foreign-reference-owner").unwrap();
    assert!(second.consume(&foreign, lease.id).await.is_err());
    let material = first.consume(&scope, lease.id).await.unwrap();
    assert_eq!(material.expose_secret(), successor);
    assert!(first.consume(&scope, lease.id).await.is_err());
    verify_secret_lifetimes(first.clone(), second.clone(), scope.clone()).await;
    verify_generation_bound_leases(first, second, scope).await;
}

async fn verify_secret_lifetimes(
    first: Arc<dyn SecretStore>,
    second: Arc<dyn SecretStore>,
    scope: ResourceScope,
) {
    let handle = SecretHandle::new("issuer-expiry-probe").unwrap();
    let expires_at = chrono::Utc::now() + chrono::Duration::seconds(3);
    first
        .put_with_expiry(
            scope.clone(),
            handle.clone(),
            "finite access value".into(),
            Some(expires_at),
        )
        .await
        .unwrap();
    let original = first.record_reference(&scope, &handle).await.unwrap();
    let first_lease = second.lease_once(&scope, &handle).await.unwrap();
    assert_eq!(first_lease.secret_expires_at, Some(expires_at));
    // Independent legacy adapters share their secret owner, but each retains
    // its own lease map. Consume through the actual issuing adapter.
    assert_eq!(
        second
            .consume(&scope, first_lease.id)
            .await
            .unwrap()
            .expose_secret(),
        "finite access value"
    );
    let waiting_lease = second.lease_once(&scope, &handle).await.unwrap();
    // Actual elapsed wall time, not a projected status flag or a forged lease.
    // This is a test fixture lifetime, not a runtime budget/default.
    let wait = (expires_at - chrono::Utc::now())
        .to_std()
        .unwrap_or_default();
    tokio::time::sleep(wait + std::time::Duration::from_millis(20)).await;
    assert!(
        second
            .consume(&scope, waiting_lease.id)
            .await
            .unwrap_err()
            .is_expired()
    );
    assert!(
        second
            .lease_once(&scope, &handle)
            .await
            .unwrap_err()
            .is_expired()
    );
    assert_eq!(
        first.record_reference(&scope, &handle).await.unwrap(),
        original,
        "expiry must not mutate the selected encrypted generation"
    );
    assert!(
        first
            .matches(&scope, &handle, &"finite access value".into())
            .await
            .unwrap()
    );

    // A new token with omitted lifetime must clear the previous deadline. Its
    // exact text remains ordinary text even when it resembles a private codec.
    let json_token = r#"{"material":"ordinary JSON token","expires_at":null}"#;
    first
        .put_with_expiry(scope.clone(), handle.clone(), json_token.into(), None)
        .await
        .unwrap();
    let unknown = second.lease_once(&scope, &handle).await.unwrap();
    assert_eq!(unknown.secret_expires_at, None);
    assert_eq!(
        second
            .consume(&scope, unknown.id)
            .await
            .unwrap()
            .expose_secret(),
        json_token
    );
    assert!(
        second.consume(&scope, waiting_lease.id).await.is_err(),
        "a replacement cannot revive an expired old lease"
    );

    first
        .put_with_expiry(
            scope.clone(),
            handle.clone(),
            "zero lifetime".into(),
            Some(chrono::Utc::now()),
        )
        .await
        .unwrap();
    assert!(
        second
            .lease_once(&scope, &handle)
            .await
            .unwrap_err()
            .is_expired()
    );
}

async fn verify_generation_bound_leases(
    first: Arc<dyn SecretStore>,
    second: Arc<dyn SecretStore>,
    scope: ResourceScope,
) {
    let handle = SecretHandle::new("lease-generation-probe").unwrap();
    for recreate in [false, true] {
        first
            .put(scope.clone(), handle.clone(), "selected material".into())
            .await
            .unwrap();
        let original = first
            .record_reference(&scope, &handle)
            .await
            .unwrap()
            .unwrap();
        let lease = first.lease_once(&scope, &handle).await.unwrap();
        if recreate {
            assert!(second.delete(&scope, &handle).await.unwrap());
        }
        // Identical bytes do not restore the selected encrypted generation.
        second
            .put(scope.clone(), handle.clone(), "selected material".into())
            .await
            .unwrap();
        let replacement = second
            .record_reference(&scope, &handle)
            .await
            .unwrap()
            .unwrap();
        assert_ne!(original, replacement);
        assert!(
            first.consume(&scope, lease.id).await.is_err(),
            "old lease cannot select replacement or recreated material"
        );
        assert_eq!(
            second.record_reference(&scope, &handle).await.unwrap(),
            Some(replacement),
            "stale consumption cannot mutate the successor"
        );
        let fresh = first.lease_once(&scope, &handle).await.unwrap();
        assert_eq!(
            first
                .consume(&scope, fresh.id)
                .await
                .unwrap()
                .expose_secret(),
            "selected material"
        );
    }
    let lease = first.lease_once(&scope, &handle).await.unwrap();
    let (a, b) = tokio::join!(
        first.consume(&scope, lease.id),
        second.consume(&scope, lease.id)
    );
    assert_eq!(
        usize::from(a.is_ok()) + usize::from(b.is_ok()),
        1,
        "one actual consumption across independent owners"
    );
    for material in [a, b].into_iter().flatten() {
        assert_eq!(material.expose_secret(), "selected material");
    }
    assert!(first.consume(&scope, lease.id).await.is_err());
    for _ in 0..4 {
        first
            .put(scope.clone(), handle.clone(), "captured before race".into())
            .await
            .unwrap();
        let lease = first.lease_once(&scope, &handle).await.unwrap();
        let (consumed, written) = tokio::join!(
            first.consume(&scope, lease.id),
            second.put(scope.clone(), handle.clone(), "concurrent successor".into()),
        );
        written.unwrap();
        if let Ok(material) = consumed {
            assert_eq!(
                material.expose_secret(),
                "captured before race",
                "a racing consume may finish with its selected generation, never the successor"
            );
        }
        assert!(
            second
                .matches(&scope, &handle, &"concurrent successor".into())
                .await
                .unwrap()
        );
    }
}

/// A selected generation is checked by the owner, not by two metadata reads
/// around an unconstrained lease. These are real owner/consume operations.
pub(super) async fn verify_selected_lease_contract(
    first: Arc<dyn SecretStore>,
    second: Arc<dyn SecretStore>,
    scope: ResourceScope,
) {
    let handle = SecretHandle::new("conditional-generation-lease").unwrap();
    assert!(
        first
            .lease_once_if_record_reference(&scope, &handle, "absent")
            .await
            .unwrap()
            .is_none()
    );
    first
        .put(
            scope.clone(),
            handle.clone(),
            "original selected material".into(),
        )
        .await
        .unwrap();
    let initial = first
        .record_reference(&scope, &handle)
        .await
        .unwrap()
        .unwrap();
    let lease = first
        .lease_once_if_record_reference(&scope, &handle, &initial)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        first
            .consume(&scope, lease.id)
            .await
            .unwrap()
            .expose_secret(),
        "original selected material"
    );
    assert!(first.consume(&scope, lease.id).await.is_err());

    let held = first
        .lease_once_if_record_reference(&scope, &handle, &initial)
        .await
        .unwrap()
        .unwrap();
    second
        .put(scope.clone(), handle.clone(), "replacement material".into())
        .await
        .unwrap();
    let ids = |leases: Vec<super::SecretLease>| {
        leases
            .into_iter()
            .map(|lease| lease.id)
            .collect::<std::collections::HashSet<_>>()
    };
    let before = ids(first.leases_for_scope(&scope).await.unwrap());
    assert!(
        first
            .lease_once_if_record_reference(&scope, &handle, &initial)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        ids(first.leases_for_scope(&scope).await.unwrap()),
        before,
        "mismatch must not create or replace a lease"
    );
    assert!(
        first.consume(&scope, held.id).await.is_err(),
        "replacement cannot flow through an old selected lease"
    );
    let replacement = first
        .record_reference(&scope, &handle)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(replacement, initial);
    let lease = first
        .lease_once_if_record_reference(&scope, &handle, &replacement)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        first
            .consume(&scope, lease.id)
            .await
            .unwrap()
            .expose_secret(),
        "replacement material"
    );

    assert!(second.delete(&scope, &handle).await.unwrap());
    assert!(
        first
            .lease_once_if_record_reference(&scope, &handle, &replacement)
            .await
            .unwrap()
            .is_none()
    );
    second
        .put(scope.clone(), handle.clone(), "replacement material".into())
        .await
        .unwrap();
    let recreated = first
        .record_reference(&scope, &handle)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(
        recreated, replacement,
        "same raw value is not the same encrypted generation"
    );
    assert!(
        first
            .lease_once_if_record_reference(&scope, &handle, &replacement)
            .await
            .unwrap()
            .is_none()
    );
    let lease = first
        .lease_once_if_record_reference(&scope, &handle, &recreated)
        .await
        .unwrap()
        .unwrap();
    first.revoke(&scope, lease.id).await.unwrap();
    assert!(first.consume(&scope, lease.id).await.is_err());
    let mut foreign = scope.clone();
    foreign.user_id = brassclaw_host_api::UserId::new("foreign-selected-lease-owner").unwrap();
    assert!(
        first
            .lease_once_if_record_reference(&foreign, &handle, &recreated)
            .await
            .unwrap()
            .is_none()
    );

    let deadline = chrono::Utc::now() + chrono::Duration::minutes(1);
    second
        .put_with_expiry(
            scope.clone(),
            handle.clone(),
            "finite selected value".into(),
            Some(deadline),
        )
        .await
        .unwrap();
    let finite = first
        .record_reference(&scope, &handle)
        .await
        .unwrap()
        .unwrap();
    let lease = first
        .lease_once_if_record_reference(&scope, &handle, &finite)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(lease.secret_expires_at, Some(deadline));
    first.revoke(&scope, lease.id).await.unwrap();
    second
        .put_with_expiry(
            scope.clone(),
            handle.clone(),
            "expired selected value".into(),
            Some(chrono::Utc::now() - chrono::Duration::seconds(1)),
        )
        .await
        .unwrap();
    let expired = first
        .record_reference(&scope, &handle)
        .await
        .unwrap()
        .unwrap();
    assert!(
        first
            .lease_once_if_record_reference(&scope, &handle, &expired)
            .await
            .unwrap_err()
            .is_expired()
    );

    let origin = SecretHandle::new("conditional-lease-original").unwrap();
    let retained = SecretHandle::new("conditional-lease-retained").unwrap();
    let original = first
        .put_original(
            scope.clone(),
            origin.clone(),
            "retained selected material".into(),
            serde_json::json!({"kind":"selected-lease-contract"}),
        )
        .await
        .unwrap()
        .unwrap();
    assert!(
        first
            .lease_once_if_record_reference(&scope, &origin, &original.reference)
            .await
            .unwrap()
            .is_none()
    );
    let output = first
        .put_retained_from_original(
            scope.clone(),
            retained.clone(),
            super::SecretCustodySource {
                scope: scope.clone(),
                handle: origin,
                reference: original.reference,
            },
        )
        .await
        .unwrap()
        .unwrap();
    let lease = first
        .lease_once_if_record_reference(&scope, &retained, &output.reference)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        first
            .consume(&scope, lease.id)
            .await
            .unwrap()
            .expose_secret(),
        "retained selected material"
    );
    first.delete(&scope, &retained).await.unwrap();
    assert!(
        first
            .lease_once_if_record_reference(&scope, &retained, &output.reference)
            .await
            .unwrap()
            .is_none()
    );
}
