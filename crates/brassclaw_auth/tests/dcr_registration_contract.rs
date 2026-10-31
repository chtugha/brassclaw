//! Flow-owner contracts; real encrypted/backend proof is tested in composition.
use brassclaw_auth::{
    AuthContinuationRef, AuthFlowId, AuthFlowManager, AuthFlowStatus, AuthProductError,
    AuthProductScope, AuthProviderId, AuthSurface, CredentialAccountLabel, DcrChallengePublication,
    DcrMaterialReference, DcrRegistrationAdmission, DcrRegistrationChallenge,
    DcrRegistrationCompletion, DcrRegistrationOriginal, DcrRegistrationOutcome,
    InMemoryAuthProductServices, OAuthAuthorizationUrl, OpaqueStateHash, PkceVerifierHash,
};
use brassclaw_host_api::{InvocationId, ResourceScope, SecretHandle, UserId};
use chrono::{Duration, Utc};

fn reference(handle: &str) -> DcrMaterialReference {
    DcrMaterialReference {
        handle: SecretHandle::new(handle).unwrap(),
        reference: format!("owner-generation-{handle}"),
    }
}
fn original() -> DcrRegistrationOriginal {
    DcrRegistrationOriginal {
        requirement_setup: None,
        scope: AuthProductScope::new(
            ResourceScope::local_default(UserId::new("dcr-user").unwrap(), InvocationId::new())
                .unwrap(),
            AuthSurface::Web,
        ),
        flow_id: AuthFlowId::new(),
        provider: AuthProviderId::new("notion").unwrap(),
        account_label: CredentialAccountLabel::new("notion-account").unwrap(),
        requested_scopes: vec![],
        continuation: AuthContinuationRef::SetupOnly,
        update_binding: None,
        opaque_state_hash: OpaqueStateHash::new("a".repeat(64)).unwrap(),
        pkce_verifier_hash: PkceVerifierHash::new("b".repeat(64)).unwrap(),
        expires_at: Utc::now() + Duration::minutes(5),
        preparation: reference("dcr-preparation"),
    }
}
fn challenge() -> DcrRegistrationChallenge {
    DcrRegistrationChallenge {
        authorization_url: OAuthAuthorizationUrl::new(
            "https://issuer.example/authorize?client_id=original",
        )
        .unwrap(),
        pkce: reference("dcr-pkce"),
        client: reference("dcr-client"),
    }
}
fn returned(action: brassclaw_auth::DcrRegistrationAction) -> DcrRegistrationCompletion {
    let submitted_at = action.submitted_at().unwrap();
    DcrRegistrationCompletion {
        dispatched: action,
        outcome: DcrRegistrationOutcome::Returned {
            submitted_at,
            status: 201,
            response: reference("dcr-response"),
        },
    }
}

#[tokio::test]
async fn local_retirement_keeps_original_plan_racing_alias_and_confirmed_receipts_immutable() {
    use brassclaw_auth::{
        DcrArtifactExpectation as Expected, DcrArtifactSlot as Slot, DcrCleanupUnavailable,
        DcrRetirementOutcome as Outcome, DcrRetirementProblem, DcrRetirementSelection,
        DcrRetirementUpdate as Update,
    };
    let auth = InMemoryAuthProductServices::new();
    let mut original = original();
    original.preparation.handle = SecretHandle::new(format!(
        "notion-oauth-dcr-flow-preparation-original-{}",
        original.flow_id
    ))
    .unwrap();
    auth.prepare_dcr_registration(original.clone())
        .await
        .unwrap();
    let plan = Slot::ALL
        .into_iter()
        .map(|slot| DcrRetirementSelection {
            slot,
            handle: original.artifact_handle(slot).unwrap(),
            expected: if slot == Slot::PreparationOriginal {
                Expected::Original {
                    reference: original.preparation.reference.clone(),
                    context: original.preparation_context(),
                }
            } else {
                Expected::Absent {
                    context: original.absent_artifact_context(slot),
                }
            },
        })
        .collect::<Vec<_>>();
    assert!(
        auth.update_dcr_retirement(&original, Update::Plan(plan.clone()))
            .await
            .is_err(),
        "active registrations cannot retire"
    );
    auth.cancel_flow(&original.scope, original.flow_id)
        .await
        .unwrap();
    assert!(
        auth.update_dcr_retirement(&original, Update::Plan(plan.clone()))
            .await
            .is_err(),
        "cancel alone is not remote confirmation"
    );
    auth.mark_dcr_cleanup_unavailable(&original, DcrCleanupUnavailable::RegistrationUnconfirmed)
        .await
        .unwrap();
    assert!(
        auth.update_dcr_retirement(&original, Update::Plan(plan[..plan.len() - 1].to_vec()))
            .await
            .is_err(),
        "every actual material slot must be covered"
    );
    let selected = auth
        .update_dcr_retirement(&original, Update::Plan(plan.clone()))
        .await
        .unwrap();
    assert_eq!(selected.action.original, original);
    let before = plan
        .iter()
        .find(|entry| entry.slot == Slot::PreparationRead)
        .unwrap()
        .clone();
    let replacement = DcrRetirementSelection {
        slot: before.slot,
        handle: before.handle.clone(),
        expected: Expected::Retained {
            reference: "auth-contract-alias-generation".into(),
            source: original.preparation.clone(),
        },
    };
    let refined = auth
        .update_dcr_retirement(
            &original,
            Update::Refine {
                before: before.clone(),
                selected: replacement.clone(),
            },
        )
        .await
        .unwrap();
    let entry = refined
        .action
        .retirement
        .as_ref()
        .unwrap()
        .entries
        .iter()
        .find(|entry| entry.selection.slot == before.slot)
        .unwrap();
    assert_eq!(entry.prior_absence.as_ref(), Some(&before));
    assert_eq!(entry.selection, replacement);
    // Losing the plan acknowledgement reads this original plus its newer exact
    // alias observation; it never restores the earlier absent selection.
    let resumed = auth
        .update_dcr_retirement(&original, Update::Plan(plan))
        .await
        .unwrap();
    assert_eq!(resumed.action.retirement, refined.action.retirement);
    for entry in refined.action.retirement.unwrap().entries {
        let outcome = Outcome::Confirmed {
            closed_reference: format!("auth-contract-closed-{}", entry.selection.slot.usage()),
        };
        auth.update_dcr_retirement(
            &original,
            Update::Record {
                selected: entry.selection,
                outcome,
            },
        )
        .await
        .unwrap();
    }
    let complete = auth
        .read_dcr_registration(&original.scope, original.flow_id)
        .await
        .unwrap()
        .unwrap();
    assert!(complete.action.retirement.as_ref().unwrap().confirmed());
    assert_eq!(
        complete.cleanup_projection().local,
        brassclaw_auth::DcrLocalCleanupStatus::Confirmed
    );
    assert!(
        auth.update_dcr_retirement(
            &original,
            Update::Record {
                selected: replacement,
                outcome: Outcome::Unresolved {
                    problem: DcrRetirementProblem::StorageUnavailable
                }
            }
        )
        .await
        .is_err(),
        "later uncertainty cannot overwrite confirmed closure"
    );
}

#[tokio::test]
async fn competing_claims_admit_exactly_one_dispatch_and_preserve_the_original() {
    let auth = InMemoryAuthProductServices::new();
    let original = original();
    let pending = auth
        .prepare_dcr_registration(original.clone())
        .await
        .unwrap();
    assert_eq!(pending.flow.status, AuthFlowStatus::Pending);
    assert!(pending.flow.challenge.is_none());
    let (one, two) = tokio::join!(
        auth.claim_dcr_registration(&original),
        auth.claim_dcr_registration(&original)
    );
    let results = [one.unwrap(), two.unwrap()];
    assert_eq!(
        results
            .iter()
            .filter(|a| matches!(a, DcrRegistrationAdmission::Dispatch(_)))
            .count(),
        1
    );
    for result in results {
        let (DcrRegistrationAdmission::Dispatch(state) | DcrRegistrationAdmission::Original(state)) =
            result;
        assert_eq!(state.action.original, original);
        assert!(matches!(
            state.action.outcome,
            DcrRegistrationOutcome::Submitted { .. }
        ));
    }
    assert!(matches!(
        auth.claim_dcr_registration(&original).await.unwrap(),
        DcrRegistrationAdmission::Original(_)
    ));
}

#[tokio::test]
async fn cancellation_retains_a_late_registration_response_without_publishing_its_url() {
    let auth = InMemoryAuthProductServices::new();
    let original = original();
    auth.prepare_dcr_registration(original.clone())
        .await
        .unwrap();
    let DcrRegistrationAdmission::Dispatch(sent) =
        auth.claim_dcr_registration(&original).await.unwrap()
    else {
        panic!("first acknowledged claim must dispatch")
    };
    auth.cancel_flow(&original.scope, original.flow_id)
        .await
        .unwrap();
    let recorded = auth
        .record_dcr_registration(&original.scope, returned(sent.action.clone()))
        .await
        .unwrap();
    assert_eq!(recorded.flow.status, AuthFlowStatus::Canceled);
    assert!(matches!(
        recorded.action.outcome,
        DcrRegistrationOutcome::Returned { .. }
    ));
    let suppressed = auth
        .publish_dcr_challenge(&original, challenge())
        .await
        .unwrap();
    assert_eq!(
        suppressed.action.publication,
        DcrChallengePublication::Suppressed
    );
    assert_eq!(suppressed.flow.status, AuthFlowStatus::Canceled);
    assert!(suppressed.flow.challenge.is_none());
    assert!(matches!(
        auth.claim_dcr_registration(&original).await.unwrap(),
        DcrRegistrationAdmission::Original(_)
    ));
    assert_eq!(
        auth.read_dcr_registration(&original.scope, original.flow_id)
            .await
            .unwrap(),
        Some(suppressed)
    );
}

#[tokio::test]
async fn changed_inputs_or_reply_cannot_replace_the_first_original() {
    let auth = InMemoryAuthProductServices::new();
    let original = original();
    let pending = auth
        .prepare_dcr_registration(original.clone())
        .await
        .unwrap();
    let mut changed = original.clone();
    changed.expires_at += Duration::seconds(1);
    assert_eq!(
        auth.prepare_dcr_registration(changed).await.unwrap_err(),
        AuthProductError::BackendConflict
    );
    assert_eq!(
        auth.read_dcr_registration(&original.scope, original.flow_id)
            .await
            .unwrap(),
        Some(pending)
    );
    let DcrRegistrationAdmission::Dispatch(sent) =
        auth.claim_dcr_registration(&original).await.unwrap()
    else {
        panic!("first acknowledged claim must dispatch")
    };
    let completion = returned(sent.action.clone());
    let recorded = auth
        .record_dcr_registration(&original.scope, completion.clone())
        .await
        .unwrap();
    assert_eq!(
        auth.record_dcr_registration(&original.scope, completion)
            .await
            .unwrap(),
        recorded
    );
    let mut contradictory = returned(sent.action);
    let DcrRegistrationOutcome::Returned {
        ref mut response, ..
    } = contradictory.outcome
    else {
        unreachable!()
    };
    *response = reference("different-response");
    assert_eq!(
        auth.record_dcr_registration(&original.scope, contradictory)
            .await
            .unwrap_err(),
        AuthProductError::BackendConflict
    );
    assert_eq!(
        auth.read_dcr_registration(&original.scope, original.flow_id)
            .await
            .unwrap(),
        Some(recorded)
    );
}

#[tokio::test]
async fn an_unknown_outcome_is_retained_and_never_reopened_for_dispatch() {
    let auth = InMemoryAuthProductServices::new();
    let original = original();
    auth.prepare_dcr_registration(original.clone())
        .await
        .unwrap();
    let DcrRegistrationAdmission::Dispatch(sent) =
        auth.claim_dcr_registration(&original).await.unwrap()
    else {
        panic!("first acknowledged claim must dispatch")
    };
    let submitted_at = sent.action.submitted_at().unwrap();
    let unknown = auth
        .record_dcr_registration(
            &original.scope,
            DcrRegistrationCompletion {
                dispatched: sent.action,
                outcome: DcrRegistrationOutcome::Unknown {
                    submitted_at,
                    error: brassclaw_auth::AuthErrorCode::BackendUnavailable,
                },
            },
        )
        .await
        .unwrap();
    assert_eq!(
        auth.prepare_dcr_registration(original.clone())
            .await
            .unwrap(),
        unknown
    );
    assert!(matches!(
        auth.claim_dcr_registration(&original).await.unwrap(),
        DcrRegistrationAdmission::Original(_)
    ));
    assert_eq!(
        auth.publish_dcr_challenge(&original, challenge())
            .await
            .unwrap_err(),
        AuthProductError::BackendConflict
    );
}

#[tokio::test]
async fn ready_publication_is_idempotent_and_does_not_repeat_registration() {
    let auth = InMemoryAuthProductServices::new();
    let original = original();
    auth.prepare_dcr_registration(original.clone())
        .await
        .unwrap();
    assert_eq!(
        auth.publish_dcr_challenge(&original, challenge())
            .await
            .unwrap_err(),
        AuthProductError::BackendConflict
    );
    let DcrRegistrationAdmission::Dispatch(sent) =
        auth.claim_dcr_registration(&original).await.unwrap()
    else {
        panic!("first acknowledged claim must dispatch")
    };
    auth.record_dcr_registration(&original.scope, returned(sent.action))
        .await
        .unwrap();
    let ready = auth
        .publish_dcr_challenge(&original, challenge())
        .await
        .unwrap();
    assert_eq!(ready.flow.status, AuthFlowStatus::AwaitingUser);
    assert_eq!(
        auth.publish_dcr_challenge(&original, challenge())
            .await
            .unwrap(),
        ready
    );
    let mut changed = challenge();
    changed.client = reference("later-client");
    assert_eq!(
        auth.publish_dcr_challenge(&original, changed)
            .await
            .unwrap_err(),
        AuthProductError::BackendConflict
    );
    let mut foreign = original.scope.clone();
    foreign.resource.invocation_id = InvocationId::new();
    assert_eq!(
        auth.read_dcr_registration(&foreign, original.flow_id)
            .await
            .unwrap_err(),
        AuthProductError::CrossScopeDenied
    );
}

#[test]
fn expiry_prevents_first_dispatch_and_suppresses_publication_without_losing_the_response() {
    let now = Utc::now();
    let mut original = original();
    original.expires_at = now + Duration::seconds(1);
    let mut flow = original.pending_flow(now).unwrap();
    let mut custody = brassclaw_auth::AuthFlowCustody::default();
    custody
        .attach_dcr_original(&flow, original.clone())
        .unwrap();
    let DcrRegistrationAdmission::Dispatch(sent) = custody
        .admit_dcr_registration(&mut flow, &original, now)
        .unwrap()
    else {
        panic!("first claim")
    };
    custody
        .record_dcr_result(&mut flow, returned(sent.action), now + Duration::seconds(2))
        .unwrap();
    let expired = custody
        .publish_dcr_challenge(
            &mut flow,
            &original,
            challenge(),
            now + Duration::seconds(2),
        )
        .unwrap();
    assert_eq!(expired.flow.status, AuthFlowStatus::Expired);
    assert_eq!(
        expired.action.publication,
        DcrChallengePublication::Suppressed
    );
    assert!(matches!(
        expired.action.outcome,
        DcrRegistrationOutcome::Returned { .. }
    ));
    let mut fresh = original.pending_flow(now).unwrap();
    let mut pending = brassclaw_auth::AuthFlowCustody::default();
    pending
        .attach_dcr_original(&fresh, original.clone())
        .unwrap();
    assert_eq!(
        pending
            .admit_dcr_registration(&mut fresh, &original, original.expires_at)
            .unwrap_err(),
        AuthProductError::UnknownOrExpiredFlow
    );
    assert!(matches!(
        pending.dcr_registration.unwrap().outcome,
        DcrRegistrationOutcome::Prepared
    ));
}

#[test]
fn cleanup_claims_never_target_a_live_account_and_only_an_empty_204_confirms_deletion() {
    use brassclaw_auth::{
        AuthFlowCustody, DcrCleanupAdmission, DcrCleanupCompletion, DcrCleanupOutcome,
        DcrCleanupPrepared,
    };
    for (status, empty_body, confirmed) in [
        (200, true, false),
        (204, false, false),
        (204, true, true),
        (401, true, false),
    ] {
        let now = Utc::now();
        let original = original();
        let mut flow = original.pending_flow(now).unwrap();
        let mut custody = AuthFlowCustody::default();
        custody
            .attach_dcr_original(&flow, original.clone())
            .unwrap();
        let DcrRegistrationAdmission::Dispatch(registration) = custody
            .admit_dcr_registration(&mut flow, &original, now)
            .unwrap()
        else {
            panic!("initial registration claim")
        };
        custody
            .record_dcr_result(&mut flow, returned(registration.action), now)
            .unwrap();
        custody
            .publish_dcr_challenge(&mut flow, &original, challenge(), now)
            .unwrap();
        let prepared = DcrCleanupPrepared {
            original: original.clone(),
            registration_response: reference("dcr-response"),
            management: reference("dcr-management"),
        };
        assert!(
            custody
                .admit_dcr_cleanup(&mut flow, prepared.clone(), now)
                .is_err(),
            "active authorization stays in use"
        );
        // The concrete owner's actual cancellation and CAS are separate native
        // acceptance requirements; this test checks pure transition decisions.
        flow.status = AuthFlowStatus::Canceled;
        custody.require_dcr_cleanup(&mut flow, now).unwrap();
        let DcrCleanupAdmission::Dispatch(dispatched) = custody
            .admit_dcr_cleanup(&mut flow, prepared.clone(), now)
            .unwrap()
        else {
            panic!("first cleanup claim")
        };
        let claim = dispatched.action.cleanup.claim().unwrap().clone();
        assert!(matches!(
            custody
                .admit_dcr_cleanup(&mut flow, prepared.clone(), now)
                .unwrap(),
            DcrCleanupAdmission::Original(_)
        ));
        let completion = DcrCleanupCompletion {
            dispatched: claim,
            outcome: DcrCleanupOutcome::Returned {
                status,
                empty_body,
                response: reference("dcr-cleanup-response"),
            },
        };
        let recorded = custody
            .record_dcr_cleanup(&flow, completion.clone(), now)
            .unwrap();
        assert_eq!(recorded.action.cleanup.remote_confirmed(), confirmed);
        assert_eq!(
            custody.record_dcr_cleanup(&flow, completion, now).unwrap(),
            recorded
        );
        assert!(matches!(
            custody.admit_dcr_cleanup(&mut flow, prepared, now).unwrap(),
            DcrCleanupAdmission::Original(_)
        ));
    }
}

#[test]
fn unknown_cleanup_is_retained_and_a_completed_account_keeps_its_registration() {
    use brassclaw_auth::{
        AuthFlowCustody, CredentialAccountId, DcrCleanupAdmission, DcrCleanupCompletion,
        DcrCleanupOutcome, DcrCleanupPrepared, DcrCleanupState,
    };
    let now = Utc::now();
    let original = original();
    let mut flow = original.pending_flow(now).unwrap();
    let mut custody = AuthFlowCustody::default();
    custody
        .attach_dcr_original(&flow, original.clone())
        .unwrap();
    let DcrRegistrationAdmission::Dispatch(registration) = custody
        .admit_dcr_registration(&mut flow, &original, now)
        .unwrap()
    else {
        panic!("initial registration claim")
    };
    custody
        .record_dcr_result(&mut flow, returned(registration.action), now)
        .unwrap();
    let prepared = DcrCleanupPrepared {
        original: original.clone(),
        registration_response: reference("dcr-response"),
        management: reference("dcr-management"),
    };
    let mut completed_flow = flow.clone();
    let mut completed_custody = custody.clone();
    completed_flow.status = AuthFlowStatus::Completed;
    completed_flow.credential_account_id = Some(CredentialAccountId::new());
    assert!(matches!(
        completed_custody
            .admit_dcr_cleanup(&mut completed_flow, prepared.clone(), now)
            .unwrap(),
        DcrCleanupAdmission::Original(_)
    ));
    assert_eq!(
        completed_custody.dcr_registration.as_ref().unwrap().cleanup,
        DcrCleanupState::Retained
    );
    flow.status = AuthFlowStatus::Canceled;
    let DcrCleanupAdmission::Dispatch(dispatched) = custody
        .admit_dcr_cleanup(&mut flow, prepared.clone(), now)
        .unwrap()
    else {
        panic!("first cleanup claim")
    };
    let claim = dispatched.action.cleanup.claim().unwrap().clone();
    let recorded = custody
        .record_dcr_cleanup(
            &flow,
            DcrCleanupCompletion {
                dispatched: claim,
                outcome: DcrCleanupOutcome::Unknown(
                    brassclaw_auth::AuthErrorCode::BackendUnavailable,
                ),
            },
            now,
        )
        .unwrap();
    assert!(!recorded.action.cleanup.remote_confirmed());
    assert!(matches!(
        recorded.action.cleanup,
        DcrCleanupState::Unknown { .. }
    ));
    let DcrCleanupAdmission::Original(retry) =
        custody.admit_dcr_cleanup(&mut flow, prepared, now).unwrap()
    else {
        panic!("unknown deletion must never be dispatched again")
    };
    assert_eq!(retry, recorded);
}

#[test]
fn dcr_usage_custody_is_pinned_and_absent_legacy_context_keeps_its_shape() {
    use brassclaw_host_api::*;
    let legacy = original();
    let wire = serde_json::to_value(&legacy).unwrap();
    let context = legacy.preparation_context();
    assert!(wire.get("requirement_setup").is_none());
    assert!(context.get("requirement_setup").is_none());
    let decoded: DcrRegistrationOriginal = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(serde_json::to_value(&decoded).unwrap(), wire);
    assert_eq!(decoded.preparation_context(), context);
    let declaration = RuntimeCredentialAccountRequirement::new(
        ExtensionId::new("notion").unwrap(),
        RuntimeCredentialRequirement {
            handle: SecretHandle::new("notion-oauth").unwrap(),
            source: RuntimeCredentialRequirementSource::ProductAuthAccount {
                provider: RuntimeCredentialAccountProviderId::new("notion").unwrap(),
                setup: RuntimeCredentialAccountSetup::OAuth { scopes: vec![] },
            },
            provider_scopes: vec![],
            audience: NetworkTargetPattern {
                scheme: None,
                host_pattern: "api.notion.com".into(),
                port: None,
            },
            target: RuntimeCredentialTarget::Header {
                name: "Authorization".into(),
                prefix: Some("Bearer ".into()),
            },
            required: true,
        },
    )
    .unwrap();
    let mut original = legacy;
    original.requirement_setup = Some(Box::new(
        brassclaw_auth::CredentialRequirementSetup::new(&original.scope, declaration.clone(), None)
            .unwrap(),
    ));
    assert_eq!(
        original.preparation_context()["format"],
        "dcr-registration-preparation/2"
    );
    let flow = original.pending_flow(Utc::now()).unwrap();
    let mut custody = brassclaw_auth::AuthFlowCustody::default();
    custody
        .attach_dcr_original(&flow, original.clone())
        .unwrap();
    assert_eq!(custody.requirement_setup, original.requirement_setup);
    assert_eq!(
        original.preparation_context()["requirement_setup"],
        serde_json::to_value(&original.requirement_setup).unwrap()
    );
    assert!(
        custody
            .dcr_original_state(&flow, &original)
            .unwrap()
            .is_some()
    );
    custody.requirement_setup = None;
    assert_eq!(
        custody.dcr_original_state(&flow, &original).unwrap_err(),
        AuthProductError::BackendConflict
    );
    let mut changed = declaration.requirement().clone();
    changed.source = RuntimeCredentialRequirementSource::ProductAuthAccount {
        provider: RuntimeCredentialAccountProviderId::new("notion").unwrap(),
        setup: RuntimeCredentialAccountSetup::ManualToken,
    };
    let changed =
        RuntimeCredentialAccountRequirement::new(ExtensionId::new("notion").unwrap(), changed)
            .unwrap();
    original.requirement_setup = Some(Box::new(
        brassclaw_auth::CredentialRequirementSetup::new(&original.scope, changed, None).unwrap(),
    ));
    assert_eq!(
        brassclaw_auth::AuthFlowCustody::default()
            .attach_dcr_original(&flow, original)
            .unwrap_err(),
        AuthProductError::BackendConflict
    );
}

#[test]
fn terminal_preparation_preserves_bound_identity_without_creating_dispatch_authority() {
    use brassclaw_auth::{
        AuthErrorCode, AuthFlowCustody, CredentialAccountId, CredentialAccountUpdateBinding,
        CredentialOwnership, DcrCleanupReason, DcrCleanupState,
    };
    let now = Utc::now();
    for reason in [DcrCleanupReason::Canceled, DcrCleanupReason::Expired] {
        let mut original = original();
        original.expires_at = now
            + if reason == DcrCleanupReason::Expired {
                Duration::seconds(-1)
            } else {
                Duration::minutes(5)
            };
        original.update_binding = Some(CredentialAccountUpdateBinding {
            account_id: CredentialAccountId::new(),
            ownership: CredentialOwnership::UserReusable,
            owner_extension: None,
            granted_extensions: vec![],
        });
        let (mut flow, mut custody) =
            AuthFlowCustody::retired_dcr_preparation(original.clone(), reason, now).unwrap();
        assert_eq!(flow.id, original.flow_id);
        assert_eq!(flow.scope, original.scope);
        assert_eq!(flow.expires_at, original.expires_at);
        assert_eq!(flow.update_binding, original.update_binding);
        assert!(custody.update_account_snapshot.is_none());
        let state = custody
            .dcr_original_state(&flow, &original)
            .unwrap()
            .unwrap();
        assert_eq!(state.action.original, original);
        assert_eq!(
            state.action.publication,
            DcrChallengePublication::Suppressed
        );
        assert!(matches!(
            state.action.outcome,
            DcrRegistrationOutcome::Prepared
        ));
        assert!(
            matches!(state.action.cleanup, DcrCleanupState::NotRegistered { reason: actual, .. } if actual == reason)
        );
        let (status, error, denial) = if reason == DcrCleanupReason::Expired {
            (
                AuthFlowStatus::Expired,
                AuthErrorCode::UnknownOrExpiredFlow,
                AuthProductError::UnknownOrExpiredFlow,
            )
        } else {
            (
                AuthFlowStatus::Canceled,
                AuthErrorCode::Canceled,
                AuthProductError::Canceled,
            )
        };
        assert_eq!(flow.status, status);
        assert_eq!(flow.error, Some(error));
        assert_eq!(
            custody
                .admit_dcr_registration(&mut flow, &original, now)
                .unwrap_err(),
            denial
        );
        flow.status = AuthFlowStatus::Pending;
        assert_eq!(
            custody.dcr_original_state(&flow, &original).unwrap_err(),
            AuthProductError::BackendConflict
        );
    }
    assert_eq!(
        original()
            .pending_flow(now + Duration::hours(1))
            .unwrap_err(),
        AuthProductError::UnknownOrExpiredFlow
    );
    assert!(
        AuthFlowCustody::retired_dcr_preparation(original(), DcrCleanupReason::Expired, now)
            .is_err()
    );
    assert!(
        AuthFlowCustody::retired_dcr_preparation(original(), DcrCleanupReason::Failed, now)
            .is_err()
    );
}

#[tokio::test]
async fn canceled_preparation_is_idempotent_without_reading_a_missing_update_account() {
    use brassclaw_auth::{
        CredentialAccountId, CredentialAccountUpdateBinding, CredentialOwnership, DcrCleanupReason,
    };
    let auth = InMemoryAuthProductServices::new();
    let mut first = original();
    first.update_binding = Some(CredentialAccountUpdateBinding {
        account_id: CredentialAccountId::new(),
        ownership: CredentialOwnership::UserReusable,
        owner_extension: None,
        granted_extensions: vec![],
    });
    let retained = auth
        .retire_dcr_preparation(first.clone(), DcrCleanupReason::Canceled)
        .await
        .unwrap();
    assert_eq!(retained.flow.status, AuthFlowStatus::Canceled);
    assert_eq!(
        auth.prepare_dcr_registration(first.clone()).await.unwrap(),
        retained
    );
    let mut changed = first.clone();
    changed.account_label = CredentialAccountLabel::new("changed original").unwrap();
    assert_eq!(
        auth.retire_dcr_preparation(changed, DcrCleanupReason::Canceled)
            .await
            .unwrap_err(),
        AuthProductError::BackendConflict
    );
    assert_eq!(
        auth.read_dcr_registration(&first.scope, first.flow_id)
            .await
            .unwrap()
            .unwrap(),
        retained
    );
    assert_eq!(
        auth.claim_dcr_registration(&first).await.unwrap_err(),
        AuthProductError::Canceled
    );
}
