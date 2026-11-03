//! Native device originals inside the existing PostgreSQL AuthFlow/SecretStore
//! owner. No database transaction spans issuer I/O. No File/ENV/session map.
use super::*;
use crate::auth_flow_custody::StoredAuthFlow;
use brassclaw_auth::Timestamp;
use brassclaw_auth::{
    NativeDeviceLoginAction, NativeDeviceLoginDefinition, NativeDeviceLoginOriginal,
    NativeDeviceOutcome, NativeDeviceRequest, NativeDeviceTransportOriginal,
    OAuthSetupMaterialReference,
};
use brassclaw_secrets::{
    SecretCustodyMetadata, SecretCustodySource, SecretCustodyState, SecretStore,
};
use secrecy::{ExposeSecret, SecretString};

/// Constructed only after the request claim's COMMIT acknowledged success.
/// Neither serde nor a plaintext field permits external fabrication/replay.
struct NativeDeviceDispatch {
    pub(crate) original: NativeDeviceLoginOriginal,
    pub(crate) request: NativeDeviceRequest,
    input: SecretCustodyMetadata,
}
pub(crate) struct NativeDeviceState {
    pub(crate) flow: AuthFlowRecord,
    pub(crate) action: NativeDeviceLoginAction,
}
/// Native one-shot display only; not a serialized flow/model/provider DTO.
pub(crate) struct NativeDeviceChallenge {
    pub(crate) user_code: SecretString,
    pub(crate) verification_uri: String,
    pub(crate) expires_at: Timestamp,
}
enum NativeDeviceAdmission {
    Dispatch(Box<NativeDeviceDispatch>),
    Original(Box<NativeDeviceLoginAction>),
}
/// Root's publisher must authenticate this in its own existing publication
/// transaction. It is a raw-original selection, not account/head publication.
pub(crate) struct NativeDeviceReady {
    pub(crate) original: NativeDeviceLoginOriginal,
    pub(crate) request: NativeDeviceRequest,
    pub(crate) response: SecretCustodySource,
    pub(crate) access_expires_at: Timestamp,
    pub(crate) request_started_at: Timestamp,
}
fn preparation_handle(id: AuthFlowId) -> Result<SecretHandle, AuthProductError> {
    SecretHandle::new(format!("codex-device-preparation-{id}"))
        .map_err(|_| AuthProductError::BackendConflict)
}
fn input_handle(id: AuthFlowId, sequence: u64) -> Result<SecretHandle, AuthProductError> {
    SecretHandle::new(format!("codex-device-input-{id}-{sequence}"))
        .map_err(|_| AuthProductError::BackendConflict)
}
pub(super) fn result_handle(
    id: AuthFlowId,
    sequence: u64,
) -> Result<SecretHandle, AuthProductError> {
    SecretHandle::new(format!("codex-device-result-{id}-{sequence}"))
        .map_err(|_| AuthProductError::BackendConflict)
}
pub(super) fn result_context(
    original: &NativeDeviceLoginOriginal,
    request: &NativeDeviceRequest,
    transport: &NativeDeviceTransportOriginal,
) -> serde_json::Value {
    serde_json::json!({"kind":"native-device-http-original/1","original":original,"request":request,"transport":transport})
}
async fn lock_native_flow(
    tx: &tokio_postgres::Transaction<'_>,
    scope: &AuthProductScope,
    id: AuthFlowId,
) -> Result<(), AuthProductError> {
    let key = serde_json::to_string(&("native-device-auth-flow", &scope.resource.tenant_id, id))
        .map_err(|_| AuthProductError::BackendUnavailable)?;
    tx.query_one(
        "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
        &[&key],
    )
    .await
    .map_err(db_err)?;
    Ok(())
}
fn selected(metadata: &SecretCustodyMetadata) -> OAuthSetupMaterialReference {
    OAuthSetupMaterialReference {
        handle: metadata.handle.clone(),
        reference: metadata.reference.clone(),
    }
}
fn action(document: &StoredAuthFlow) -> Result<&NativeDeviceLoginAction, AuthProductError> {
    let action = document
        .auth_custody
        .device_login
        .as_deref()
        .ok_or(AuthProductError::BackendConflict)?;
    action.verify_flow(&document.flow)?;
    Ok(action)
}
impl PgAuthProductServices {
    /// Immutable originals are not runtime credentials. Make the existing
    /// exact-source retained output inside the caller's transaction, then use
    /// a one-shot lease of THAT output. Callers authenticate the original's
    /// protocol context before entering this private material boundary.
    async fn read_native_device_original_in_transaction(
        &self,
        tx: &tokio_postgres::Transaction<'_>,
        source: &SecretCustodySource,
    ) -> Result<SecretString, AuthProductError> {
        let handle = SecretHandle::new(format!("{}-retained-read", source.handle.as_str()))
            .map_err(|_| AuthProductError::BackendConflict)?;
        let retained = self
            .secret_store
            .put_retained_from_original_in_transaction(
                tx,
                source.scope.clone(),
                handle,
                source.clone(),
            )
            .await
            .map_err(|_| AuthProductError::BackendUnavailable)?
            .ok_or(AuthProductError::BackendConflict)?;
        let lease = self
            .secret_store
            .lease_once_if_record_reference_in_transaction(
                tx,
                &source.scope,
                &retained.handle,
                &retained.reference,
            )
            .await
            .map_err(|_| AuthProductError::BackendUnavailable)?
            .ok_or(AuthProductError::BackendConflict)?;
        self.secret_store
            .consume_in_transaction(tx, &source.scope, lease.id)
            .await
            .map_err(|_| AuthProductError::BackendUnavailable)
    }

    async fn authenticate_device_source(
        &self,
        tx: &tokio_postgres::Transaction<'_>,
        original: &NativeDeviceLoginOriginal,
        reference: &OAuthSetupMaterialReference,
        context: serde_json::Value,
    ) -> Result<SecretCustodyMetadata, AuthProductError> {
        let actual = self
            .secret_store
            .custody_in_transaction(tx, &original.definition.scope.resource, &reference.handle)
            .await
            .map_err(|_| AuthProductError::BackendUnavailable)?
            .ok_or(AuthProductError::BackendConflict)?;
        if selected(&actual) != *reference
            || actual.scope != original.definition.scope.resource
            || actual.state != (SecretCustodyState::Original { context })
        {
            return Err(AuthProductError::BackendConflict);
        }
        Ok(actual)
    }
    /// Same transaction retains encrypted first intent/client context and creates
    /// its true scoped flow. Repeats compare actual original; labels grant nothing.
    pub(crate) async fn prepare_native_device(
        &self,
        definition: NativeDeviceLoginDefinition,
        material: SecretString,
    ) -> Result<NativeDeviceLoginAction, AuthProductError> {
        definition.validate()?;
        if definition.created_at > Utc::now() {
            return Err(AuthProductError::BackendConflict);
        }
        let mut client = self.pool.get().await.map_err(pool_err)?;
        let tx = client.transaction().await.map_err(db_err)?;
        lock_native_flow(&tx, &definition.scope, definition.flow_id).await?;
        let existing=tx.query_opt("SELECT data FROM brassclaw_product_auth_flows WHERE tenant_id=$1 AND id=$2 FOR UPDATE",
            &[&definition.scope.resource.tenant_id.as_str(),&definition.flow_id.to_string()]).await.map_err(db_err)?;
        let metadata = self
            .secret_store
            .put_original_in_transaction(
                &tx,
                definition.scope.resource.clone(),
                preparation_handle(definition.flow_id)?,
                material,
                definition.preparation_context()?,
            )
            .await
            .map_err(|_| AuthProductError::BackendUnavailable)?
            .ok_or(AuthProductError::BackendConflict)?;
        let original = NativeDeviceLoginOriginal {
            definition,
            preparation: selected(&metadata),
        };
        if let Some(row) = existing {
            let document: StoredAuthFlow = serde_json::from_value(row.get(0))
                .map_err(|_| AuthProductError::BackendUnavailable)?;
            let existing = action(&document)?;
            if existing.original != original {
                return Err(AuthProductError::BackendConflict);
            }
            let existing = existing.clone();
            tx.commit().await.map_err(db_err)?;
            return Ok(existing);
        }
        // Missing history must not resurrect an already submitted first request.
        if self
            .secret_store
            .inspect_record_reference_in_transaction(
                &tx,
                &original.definition.scope.resource,
                &input_handle(original.definition.flow_id, 0)?,
            )
            .await
            .map_err(|_| AuthProductError::BackendUnavailable)?
            .is_some()
        {
            return Err(AuthProductError::BackendConflict);
        }
        if original.definition.expires_at <= Utc::now() {
            return Err(AuthProductError::UnknownOrExpiredFlow);
        }
        let flow = original.awaiting_flow()?;
        let action = NativeDeviceLoginAction::new(original)?;
        let mut document = StoredAuthFlow::new(flow, None)?;
        document.auth_custody.device_login = Some(Box::new(action.clone()));
        let data =
            serde_json::to_value(&document).map_err(|_| AuthProductError::BackendUnavailable)?;
        tx.execute("INSERT INTO brassclaw_product_auth_flows (tenant_id,id,user_id,data) VALUES ($1,$2,$3,$4)",
            &[&document.flow.scope.resource.tenant_id.as_str(),&document.flow.id.to_string(),&document.flow.scope.resource.user_id.as_str(),&data]).await.map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        Ok(action)
    }
    pub(crate) async fn read_native_device_state(
        &self,
        scope: &AuthProductScope,
        id: AuthFlowId,
    ) -> Result<Option<NativeDeviceState>, AuthProductError> {
        let mut client = self.pool.get().await.map_err(pool_err)?;
        let tx = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(db_err)?;
        let row = tx
            .query_opt(
                "SELECT data FROM brassclaw_product_auth_flows WHERE tenant_id=$1 AND id=$2",
                &[&scope.resource.tenant_id.as_str(), &id.to_string()],
            )
            .await
            .map_err(db_err)?;
        let Some(row) = row else {
            // Missing state is genuinely absent only if no durable original or
            // submitted first request exists. Corrupt/lost history cannot restart.
            for handle in [preparation_handle(id)?, input_handle(id, 0)?] {
                if self
                    .secret_store
                    .inspect_record_reference_in_transaction(&tx, &scope.resource, &handle)
                    .await
                    .map_err(|_| AuthProductError::BackendUnavailable)?
                    .is_some()
                {
                    return Err(AuthProductError::BackendConflict);
                }
            }
            tx.commit().await.map_err(db_err)?;
            return Ok(None);
        };
        let document = super::callback::decode_scoped_flow(scope, id, row.get(0))?;
        let actual = action(&document)?;
        self.authenticate_device_source(
            &tx,
            &actual.original,
            &actual.original.preparation,
            actual.original.definition.preparation_context()?,
        )
        .await?;
        let result = NativeDeviceState {
            flow: document.flow.clone(),
            action: actual.clone(),
        };
        tx.commit().await.map_err(db_err)?;
        Ok(Some(result))
    }
    /// Recover only this exact encrypted native preparation in the caller's
    /// transaction. Reading a completed/canceled original never authorizes I/O.
    /// Caller must acknowledge COMMIT before using it for a new external action.
    pub(crate) async fn native_device_preparation_in_transaction(
        &self,
        tx: &tokio_postgres::Transaction<'_>,
        original: &NativeDeviceLoginOriginal,
    ) -> Result<SecretString, AuthProductError> {
        original.validate()?;
        let scope = &original.definition.scope;
        let row = tx
            .query_opt(
                "SELECT data FROM brassclaw_product_auth_flows WHERE tenant_id=$1 AND id=$2",
                &[
                    &scope.resource.tenant_id.as_str(),
                    &original.definition.flow_id.to_string(),
                ],
            )
            .await
            .map_err(db_err)?
            .ok_or(AuthProductError::BackendConflict)?;
        let document =
            super::callback::decode_scoped_flow(scope, original.definition.flow_id, row.get(0))?;
        if action(&document)?.original != *original {
            return Err(AuthProductError::BackendConflict);
        }
        let metadata = self
            .authenticate_device_source(
                tx,
                original,
                &original.preparation,
                original.definition.preparation_context()?,
            )
            .await?;
        self.read_native_device_original_in_transaction(tx, &source_from(&metadata))
            .await
    }
    async fn claim_native_device(
        &self,
        original: &NativeDeviceLoginOriginal,
        id: uuid::Uuid,
    ) -> Result<NativeDeviceAdmission, AuthProductError> {
        let scope = &original.definition.scope;
        let flow_id = original.definition.flow_id;
        let mut client = self.pool.get().await.map_err(pool_err)?;
        let tx = client.transaction().await.map_err(db_err)?;
        lock_native_flow(&tx, scope, flow_id).await?;
        let (mut document, revision) = super::callback::locked_flow(&tx, scope, flow_id).await?;
        let actual = action(&document)?;
        if actual.original != *original {
            return Err(AuthProductError::BackendConflict);
        }
        self.authenticate_device_source(
            &tx,
            original,
            &original.preparation,
            original.definition.preparation_context()?,
        )
        .await?;
        if matches!(
            document.flow.status,
            AuthFlowStatus::Completed
                | AuthFlowStatus::Canceled
                | AuthFlowStatus::Failed
                | AuthFlowStatus::Expired
        ) {
            let actual = actual.clone();
            tx.rollback().await.map_err(db_err)?;
            return Ok(NativeDeviceAdmission::Original(Box::new(actual)));
        }
        // Setup expiry stops NEW issuer requests, not custody/publication of
        // a once-authorized token request already in flight or already known.
        if actual.requests.last().is_some_and(|request| {
            request.kind == brassclaw_auth::NativeDeviceRequestKind::TokenExchange
        }) {
            let actual = actual.clone();
            tx.rollback().await.map_err(db_err)?;
            return Ok(NativeDeviceAdmission::Original(Box::new(actual)));
        }
        if Utc::now() >= actual.expiry() {
            document.flow.status = AuthFlowStatus::Expired;
            document.flow.error = Some(brassclaw_auth::AuthErrorCode::UnknownOrExpiredFlow);
            document.flow.updated_at = Utc::now();
            super::callback::save_flow(&tx, &document, revision).await?;
            let actual = action(&document)?.clone();
            tx.commit().await.map_err(db_err)?;
            return Ok(NativeDeviceAdmission::Original(Box::new(actual)));
        }
        let request = document
            .auth_custody
            .device_login
            .as_mut()
            .ok_or(AuthProductError::BackendConflict)?
            .claim(id, Utc::now())?;
        let Some(request) = request else {
            let actual = action(&document)?.clone();
            tx.rollback().await.map_err(db_err)?;
            return Ok(NativeDeviceAdmission::Original(Box::new(actual)));
        };
        let source_context = if request.sequence == 0 {
            original.definition.preparation_context()?
        } else {
            let previous = action(&document)?
                .requests
                .iter()
                .find(|r| match &r.outcome {
                    NativeDeviceOutcome::Challenge { response, .. }
                    | NativeDeviceOutcome::Authorized { response, .. } => {
                        response == &request.source
                    }
                    _ => false,
                })
                .ok_or(AuthProductError::BackendConflict)?;
            let mut submitted = previous.clone();
            submitted.outcome = NativeDeviceOutcome::Submitted;
            result_context(
                original,
                &submitted,
                transport_original(action(&document)?, previous)?,
            )
        };
        let source = self
            .authenticate_device_source(&tx, original, &request.source, source_context)
            .await?;
        let input = self
            .secret_store
            .put_retained_from_original_in_transaction(
                &tx,
                scope.resource.clone(),
                input_handle(flow_id, request.sequence)?,
                SecretCustodySource {
                    scope: source.scope,
                    handle: source.handle,
                    reference: source.reference,
                },
            )
            .await
            .map_err(|_| AuthProductError::BackendUnavailable)?
            .ok_or(AuthProductError::BackendConflict)?;
        document.flow.status = AuthFlowStatus::Completing;
        document.flow.updated_at = request.submitted_at;
        super::callback::save_flow(&tx, &document, revision).await?;
        tx.commit().await.map_err(db_err)?;
        Ok(NativeDeviceAdmission::Dispatch(Box::new(
            NativeDeviceDispatch {
                original: original.clone(),
                request,
                input,
            },
        )))
    }
    async fn consume_native_device_input(
        &self,
        dispatch: &NativeDeviceDispatch,
    ) -> Result<SecretString, AuthProductError> {
        let scope = &dispatch.original.definition.scope.resource;
        let expected = SecretCustodyState::Retained {
            source: SecretCustodySource {
                scope: scope.clone(),
                handle: dispatch.request.source.handle.clone(),
                reference: dispatch.request.source.reference.clone(),
            },
        };
        if dispatch.input.scope != *scope
            || dispatch.input.handle
                != input_handle(
                    dispatch.original.definition.flow_id,
                    dispatch.request.sequence,
                )?
            || dispatch.input.state != expected
        {
            return Err(AuthProductError::BackendConflict);
        }
        let lease = self
            .secret_store
            .lease_once_if_record_reference(
                scope,
                &dispatch.input.handle,
                &dispatch.input.reference,
            )
            .await
            .map_err(|_| AuthProductError::BackendUnavailable)?
            .ok_or(AuthProductError::BackendConflict)?;
        self.secret_store
            .consume(scope, lease.id)
            .await
            .map_err(|_| AuthProductError::BackendUnavailable)
    }
}

/// Only run_native_device_once can create this sink, from its acknowledged
/// private dispatch and consumed selected input. No serde DTO can request it.
struct NativeDeviceCustodian<'a> {
    auth: &'a PgAuthProductServices,
    dispatch: &'a NativeDeviceDispatch,
    expected: brassclaw_host_api::RuntimeHttpEgressRequest,
    transport: Option<NativeDeviceTransportOriginal>,
}
#[async_trait::async_trait]
impl brassclaw_host_runtime::HostHttpOriginalCustody for NativeDeviceCustodian<'_> {
    async fn authenticate_request(
        &mut self,
        actual: &brassclaw_host_api::RuntimeHttpEgressRequest,
    ) -> Result<(), brassclaw_host_runtime::HostHttpOriginalCustodyError> {
        use brassclaw_host_runtime::HostHttpOriginalCustodyError as CustodyError;
        if self.transport.is_some() {
            return Err(CustodyError);
        }
        // Host's already-staged authoritative network policy may replace the
        // request fallback. Every other final host-request field must match.
        let mut expected = self.expected.clone();
        expected.network_policy = actual.network_policy.clone();
        if expected != *actual
            || actual.scope != self.dispatch.original.definition.scope.resource
            || !actual.credential_injections.is_empty()
            || actual.save_body_to.is_some()
        {
            return Err(CustodyError);
        }
        self.transport = Some(
            self.auth
                .register_native_device_transport(self.dispatch, actual)
                .await
                .map_err(|_| CustodyError)?,
        );
        Ok(())
    }
    async fn retain(
        self: Box<Self>,
        original: brassclaw_secrets::SecretMaterial,
    ) -> Result<(), brassclaw_host_runtime::HostHttpOriginalCustodyError> {
        use brassclaw_host_runtime::HostHttpOriginalCustodyError as CustodyError;
        let transport = self.transport.as_ref().ok_or(CustodyError)?;
        let response: NativeDeviceNetworkOriginal =
            serde_json::from_str(original.expose_secret()).map_err(|_| CustodyError)?;
        let raw = NativeDeviceRawResponse {
            request_started_at: transport.request_started_at,
            received_at: Utc::now(),
            response,
        };
        self.auth
            .record_native_device_return(self.dispatch, raw)
            .await
            .map_err(|_| CustodyError)?;
        Ok(())
    }
}
impl PgAuthProductServices {
    /// Commit the actual final Host request Original BEFORE permitting transport.
    /// A second preflight, even with identical bytes, cannot submit another call.
    async fn register_native_device_transport(
        &self,
        dispatch: &NativeDeviceDispatch,
        request: &brassclaw_host_api::RuntimeHttpEgressRequest,
    ) -> Result<NativeDeviceTransportOriginal, AuthProductError> {
        let original = &dispatch.original;
        let scope = &original.definition.scope;
        let mut client = self.pool.get().await.map_err(pool_err)?;
        let tx = client.transaction().await.map_err(db_err)?;
        lock_native_flow(&tx, scope, original.definition.flow_id).await?;
        let (mut document, revision) =
            super::callback::locked_flow(&tx, scope, original.definition.flow_id).await?;
        let current = action(&document)?;
        if current.original != *original
            || current.requests.last() != Some(&dispatch.request)
            || current
                .transport_originals
                .iter()
                .any(|entry| entry.request_id == dispatch.request.id)
            || document.flow.status != AuthFlowStatus::Completing
        {
            return Err(AuthProductError::BackendConflict);
        }
        let request_started_at = Utc::now();
        if request_started_at >= current.expiry() {
            return Err(AuthProductError::UnknownOrExpiredFlow);
        }
        let expiry = current.expiry();
        let handle = transport_handle(original.definition.flow_id, dispatch.request.sequence)?;
        if self
            .secret_store
            .inspect_record_reference_in_transaction(&tx, &scope.resource, &handle)
            .await
            .map_err(|_| AuthProductError::BackendUnavailable)?
            .is_some()
        {
            return Err(AuthProductError::BackendConflict);
        }
        let material = SecretString::from(
            serde_json::to_string(request).map_err(|_| AuthProductError::BackendUnavailable)?,
        );
        let actual = self
            .secret_store
            .put_original_in_transaction(
                &tx,
                scope.resource.clone(),
                handle,
                material,
                request_context(original, &dispatch.request),
            )
            .await
            .map_err(|_| AuthProductError::BackendUnavailable)?
            .ok_or(AuthProductError::BackendConflict)?;
        let selected = NativeDeviceTransportOriginal {
            request_id: dispatch.request.id,
            source: selected(&actual),
            request_started_at,
        };
        document
            .auth_custody
            .device_login
            .as_mut()
            .ok_or(AuthProductError::BackendConflict)?
            .transport_originals
            .push(selected.clone());
        super::callback::save_flow(&tx, &document, revision).await?;
        tx.commit().await.map_err(db_err)?;
        // Slow commit/ACK must not grant another issuer request after expiry.
        if Utc::now() >= expiry {
            return Err(AuthProductError::UnknownOrExpiredFlow);
        }
        Ok(selected)
    }
}

// This private codec is used ONLY inside encrypted SecretStore originals. It is
// not a route/SDK/model DTO and no unfiltered Debug is implemented.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeDeviceRawResponse {
    request_started_at: Timestamp,
    received_at: Timestamp,
    response: NativeDeviceNetworkOriginal,
}
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct NativeDeviceNetworkUsage {
    request_bytes: u64,
    response_bytes: u64,
    resolved_ip: Option<std::net::IpAddr>,
}
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct NativeDeviceNetworkOriginal {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    usage: NativeDeviceNetworkUsage,
}
fn transport_handle(id: AuthFlowId, sequence: u64) -> Result<SecretHandle, AuthProductError> {
    SecretHandle::new(format!("codex-device-request-{id}-{sequence}"))
        .map_err(|_| AuthProductError::BackendConflict)
}
fn request_context(
    original: &NativeDeviceLoginOriginal,
    request: &NativeDeviceRequest,
) -> serde_json::Value {
    serde_json::json!({"kind":"native-device-http-request/1","original":original,"request":request})
}
fn transport_original<'a>(
    action: &'a NativeDeviceLoginAction,
    request: &NativeDeviceRequest,
) -> Result<&'a NativeDeviceTransportOriginal, AuthProductError> {
    let mut matches = action
        .transport_originals
        .iter()
        .filter(|entry| entry.request_id == request.id);
    let selected = matches.next().ok_or(AuthProductError::BackendConflict)?;
    if matches.next().is_some()
        || selected.source.handle
            != transport_handle(action.original.definition.flow_id, request.sequence)?
    {
        return Err(AuthProductError::BackendConflict);
    }
    Ok(selected)
}
fn raw_from_material(value: &SecretString) -> Result<NativeDeviceRawResponse, AuthProductError> {
    use secrecy::ExposeSecret;
    serde_json::from_str(value.expose_secret()).map_err(|_| AuthProductError::BackendConflict)
}
fn source_from(metadata: &SecretCustodyMetadata) -> SecretCustodySource {
    SecretCustodySource {
        scope: metadata.scope.clone(),
        handle: metadata.handle.clone(),
        reference: metadata.reference.clone(),
    }
}
fn checked_after(at: Timestamp, seconds: u64) -> Result<Timestamp, AuthProductError> {
    at.checked_add_signed(
        chrono::Duration::try_seconds(
            i64::try_from(seconds).map_err(|_| AuthProductError::BackendConflict)?,
        )
        .ok_or(AuthProductError::BackendConflict)?,
    )
    .ok_or(AuthProductError::BackendConflict)
}
fn device_outcome(
    action: &NativeDeviceLoginAction,
    request: &NativeDeviceRequest,
    raw: &NativeDeviceRawResponse,
    response: OAuthSetupMaterialReference,
) -> Result<NativeDeviceOutcome, AuthProductError> {
    use brassclaw_auth::NativeDeviceRequestKind;
    if raw.request_started_at < request.submitted_at || raw.received_at < raw.request_started_at {
        return Err(AuthProductError::BackendConflict);
    }
    let status = raw.response.status;
    match request.kind {
        NativeDeviceRequestKind::UserCode if (200..300).contains(&status) => {
            let (parsed, issuer_expires_at) =
                brassclaw_llm::parse_codex_device_start_with_issuer_expiry(
                    &raw.response.body,
                    raw.request_started_at,
                )
                .map_err(|_| AuthProductError::MalformedCallback)?;
            let (_, _, verification_uri, interval, _) = parsed.into_native_parts();
            let expires_at = issuer_expires_at
                .unwrap_or(action.original.definition.expires_at)
                .min(action.original.definition.expires_at);
            let uri = url::Url::parse(&verification_uri)
                .map_err(|_| AuthProductError::MalformedCallback)?;
            if !matches!(uri.scheme(), "http" | "https")
                || uri.host_str().is_none()
                || !uri.username().is_empty()
                || uri.password().is_some()
                || uri.fragment().is_some()
            {
                return Err(AuthProductError::MalformedCallback);
            }
            Ok(NativeDeviceOutcome::Challenge {
                response,
                issuer_expires_at,
                expires_at,
                next_poll_at: checked_after(raw.received_at, interval)?.min(expires_at),
                interval_secs: interval,
            })
        }
        NativeDeviceRequestKind::Poll if status == 403 || status == 429 => {
            let previous = action
                .requests
                .iter()
                .rev()
                .find_map(|r| match r.outcome {
                    NativeDeviceOutcome::Challenge { interval_secs, .. }
                    | NativeDeviceOutcome::Pending { interval_secs, .. } => Some(interval_secs),
                    _ => None,
                })
                .ok_or(AuthProductError::BackendConflict)?;
            let mut interval = if status == 429 {
                previous
                    .checked_add(brassclaw_llm::codex_device_minimum_interval_secs())
                    .ok_or(AuthProductError::BackendConflict)?
            } else {
                previous
            };
            // Actual issuer Retry-After is a lower bound. Do not copy the old
            // File manager's arbitrary 60-second client backoff cap.
            for (_, value) in raw
                .response
                .headers
                .iter()
                .filter(|(key, _)| key.eq_ignore_ascii_case("retry-after"))
            {
                let delay = match value.parse::<u64>() {
                    Ok(seconds) => seconds,
                    Err(_) => {
                        let date = chrono::DateTime::parse_from_rfc2822(value)
                            .map_err(|_| AuthProductError::MalformedCallback)?;
                        u64::try_from(
                            date.signed_duration_since(raw.received_at)
                                .num_seconds()
                                .max(0),
                        )
                        .map_err(|_| AuthProductError::BackendConflict)?
                    }
                };
                interval = interval.max(delay);
            }
            Ok(NativeDeviceOutcome::Pending {
                response,
                next_poll_at: checked_after(raw.received_at, interval)?.min(action.expiry()),
                interval_secs: interval,
            })
        }
        NativeDeviceRequestKind::Poll if (200..300).contains(&status) => {
            let parsed = brassclaw_llm::parse_codex_device_authorization(&raw.response.body)
                .map_err(|_| AuthProductError::MalformedCallback)?;
            let code = brassclaw_auth::OAuthAuthorizationCode::new(parsed.authorization_code)?;
            let verifier = brassclaw_auth::PkceVerifierSecret::new(parsed.code_verifier)?;
            // The existing Device response declares no challenge method. Keep
            // the actual issuer-returned verifier/code and their hashes; do not
            // invent browser-PKCE fields or assume another challenge algorithm.
            Ok(NativeDeviceOutcome::Authorized {
                response,
                authorization_code_hash: brassclaw_auth::authorization_code_hash(&code)?,
                pkce_verifier_hash: brassclaw_auth::pkce_verifier_hash(&verifier)?,
            })
        }
        NativeDeviceRequestKind::TokenExchange if (200..300).contains(&status) => {
            let tokens = brassclaw_llm::parse_codex_device_tokens(
                &raw.response.body,
                raw.request_started_at,
            )
            .map_err(|_| AuthProductError::TokenExchangeFailed)?;
            Ok(NativeDeviceOutcome::Tokens {
                response,
                request_started_at: raw.request_started_at,
                access_expires_at: tokens.expires_at,
                has_refresh_token: !tokens.refresh_token.expose_secret().is_empty(),
            })
        }
        _ => Ok(NativeDeviceOutcome::Rejected {
            response,
            error: if request.kind == NativeDeviceRequestKind::Poll && status == 404 {
                brassclaw_auth::AuthErrorCode::ProviderDenied
            } else {
                brassclaw_auth::AuthErrorCode::TokenExchangeFailed
            },
        }),
    }
}
impl PgAuthProductServices {
    async fn record_native_device_return(
        &self,
        dispatch: &NativeDeviceDispatch,
        raw: NativeDeviceRawResponse,
    ) -> Result<NativeDeviceLoginAction, AuthProductError> {
        let original = &dispatch.original;
        let scope = &original.definition.scope;
        let id = original.definition.flow_id;
        let mut client = self.pool.get().await.map_err(pool_err)?;
        let tx = client.transaction().await.map_err(db_err)?;
        lock_native_flow(&tx, scope, id).await?;
        let (mut document, revision) = super::callback::locked_flow(&tx, scope, id).await?;
        let current = action(&document)?;
        let transport = transport_original(current, &dispatch.request)?.clone();
        if raw.request_started_at != transport.request_started_at {
            return Err(AuthProductError::BackendConflict);
        }
        self.authenticate_device_source(
            &tx,
            original,
            &transport.source,
            request_context(original, &dispatch.request),
        )
        .await?;
        if current.original != *original {
            return Err(AuthProductError::BackendConflict);
        }
        let mut expected = current
            .requests
            .get(
                usize::try_from(dispatch.request.sequence)
                    .map_err(|_| AuthProductError::BackendConflict)?,
            )
            .ok_or(AuthProductError::BackendConflict)?
            .clone();
        expected.outcome = NativeDeviceOutcome::Submitted;
        if expected != dispatch.request {
            return Err(AuthProductError::BackendConflict);
        }
        // Original raw bytes + known result + lifecycle status have one commit;
        // publication is later and must not delete/replace this retained return.
        let material = SecretString::from(
            serde_json::to_string(&raw).map_err(|_| AuthProductError::BackendUnavailable)?,
        );
        let metadata = self
            .secret_store
            .put_original_in_transaction(
                &tx,
                scope.resource.clone(),
                result_handle(id, dispatch.request.sequence)?,
                material,
                result_context(original, &dispatch.request, &transport),
            )
            .await
            .map_err(|_| AuthProductError::BackendUnavailable)?
            .ok_or(AuthProductError::BackendConflict)?;
        let response = selected(&metadata);
        let outcome = device_outcome(current, &dispatch.request, &raw, response.clone())
            .unwrap_or_else(|error| NativeDeviceOutcome::Unknown {
                response: Some(response),
                error: error.code(),
            });
        self.apply_native_outcome(&tx, &mut document, revision, &dispatch.request, outcome)
            .await?;
        let result = action(&document)?.clone();
        tx.commit().await.map_err(db_err)?;
        Ok(result)
    }
    async fn apply_native_outcome(
        &self,
        tx: &tokio_postgres::Transaction<'_>,
        document: &mut StoredAuthFlow,
        revision: i64,
        dispatched: &NativeDeviceRequest,
        outcome: NativeDeviceOutcome,
    ) -> Result<(), AuthProductError> {
        let action = document
            .auth_custody
            .device_login
            .as_mut()
            .ok_or(AuthProductError::BackendConflict)?;
        let before = action.clone();
        action.record(dispatched, outcome.clone())?;
        if **action == *before {
            return Ok(());
        }
        document.flow.expires_at = action.expiry();
        if !matches!(
            document.flow.status,
            AuthFlowStatus::Canceled | AuthFlowStatus::Expired | AuthFlowStatus::Completed
        ) {
            document.flow.status = match outcome {
                NativeDeviceOutcome::Challenge { .. } | NativeDeviceOutcome::Pending { .. } => {
                    AuthFlowStatus::AwaitingUser
                }
                NativeDeviceOutcome::Authorized { .. } | NativeDeviceOutcome::Tokens { .. } => {
                    AuthFlowStatus::Completing
                }
                _ => AuthFlowStatus::Failed,
            };
            document.flow.error = match outcome {
                NativeDeviceOutcome::Unknown { error, .. }
                | NativeDeviceOutcome::Rejected { error, .. } => Some(error),
                _ => None,
            };
        }
        document.flow.updated_at = Utc::now();
        super::callback::save_flow(tx, document, revision).await
    }
    async fn known_native_device_return(
        &self,
        dispatch: &NativeDeviceDispatch,
    ) -> Result<Option<NativeDeviceLoginAction>, AuthProductError> {
        let state = self
            .read_native_device_state(
                &dispatch.original.definition.scope,
                dispatch.original.definition.flow_id,
            )
            .await?
            .ok_or(AuthProductError::BackendConflict)?;
        if state.action.original != dispatch.original {
            return Err(AuthProductError::BackendConflict);
        }
        let actual = state
            .action
            .requests
            .get(
                usize::try_from(dispatch.request.sequence)
                    .map_err(|_| AuthProductError::BackendConflict)?,
            )
            .ok_or(AuthProductError::BackendConflict)?;
        let mut expected = actual.clone();
        expected.outcome = NativeDeviceOutcome::Submitted;
        if expected != dispatch.request {
            return Err(AuthProductError::BackendConflict);
        }
        if matches!(actual.outcome, NativeDeviceOutcome::Submitted) {
            Ok(None)
        } else {
            Ok(Some(state.action))
        }
    }
    async fn record_native_device_unknown(
        &self,
        dispatch: &NativeDeviceDispatch,
        error: AuthProductError,
    ) -> Result<NativeDeviceLoginAction, AuthProductError> {
        let original = &dispatch.original;
        let scope = &original.definition.scope;
        let id = original.definition.flow_id;
        let mut client = self.pool.get().await.map_err(pool_err)?;
        let tx = client.transaction().await.map_err(db_err)?;
        lock_native_flow(&tx, scope, id).await?;
        let (mut document, revision) = super::callback::locked_flow(&tx, scope, id).await?;
        if action(&document)?.original != *original {
            return Err(AuthProductError::BackendConflict);
        }
        self.apply_native_outcome(
            &tx,
            &mut document,
            revision,
            &dispatch.request,
            NativeDeviceOutcome::Unknown {
                response: None,
                error: error.code(),
            },
        )
        .await?;
        let result = action(&document)?.clone();
        tx.commit().await.map_err(db_err)?;
        Ok(result)
    }
    /// One acknowledged original request only. There is no hidden poll loop,
    /// automatic restart/new login ID or DB transaction over the issuer call.
    pub(crate) async fn run_native_device_once(
        &self,
        transport: &crate::oauth_provider_client::HostOAuthProviderClient,
        original: &NativeDeviceLoginOriginal,
        request_id: uuid::Uuid,
    ) -> Result<NativeDeviceLoginAction, AuthProductError> {
        use brassclaw_auth::NativeDeviceRequestKind;
        use secrecy::ExposeSecret;
        // Original destinations were selected once using this same existing
        // callback convention; retries cannot substitute a fresh token handle.
        if transport.native_device_destinations(
            original.definition.flow_id,
            original.definition.scope.resource.invocation_id,
        )? != original.definition.destinations
        {
            return Err(AuthProductError::BackendConflict);
        }
        let original_egress = transport.native_original_egress()?;
        let dispatch = match self.claim_native_device(original, request_id).await? {
            NativeDeviceAdmission::Original(original) => return Ok(*original),
            NativeDeviceAdmission::Dispatch(dispatch) => dispatch,
        };
        let prepared:Result<(String,&str,Vec<u8>),AuthProductError>=async {
            let material=self.consume_native_device_input(&dispatch).await?;
            let auth=original.definition.client.auth_endpoint.trim_end_matches('/');
            match dispatch.request.kind {
                NativeDeviceRequestKind::UserCode=>Ok((format!("{auth}/api/accounts/deviceauth/usercode"),"application/json",
                    serde_json::to_vec(&serde_json::json!({"client_id":original.definition.client.client_id})).map_err(|_|AuthProductError::BackendUnavailable)?)),
                NativeDeviceRequestKind::Poll=>{
                    let raw=raw_from_material(&material)?;
                    let parsed=brassclaw_llm::parse_codex_device_start(&raw.response.body,raw.received_at).map_err(|_|AuthProductError::MalformedCallback)?;
                    let (device,user,_,_,_)=parsed.into_native_parts();
                    Ok((format!("{auth}/api/accounts/deviceauth/token"),"application/json",
                        serde_json::to_vec(&serde_json::json!({"device_auth_id":device.expose_secret(),"user_code":user.expose_secret()}))
                            .map_err(|_|AuthProductError::BackendUnavailable)?))
                }
                NativeDeviceRequestKind::TokenExchange=>{
                    let raw=raw_from_material(&material)?;
                    let authorization=brassclaw_llm::parse_codex_device_authorization(&raw.response.body).map_err(|_|AuthProductError::MalformedCallback)?;
                    let code=brassclaw_auth::OAuthAuthorizationCode::new(authorization.authorization_code)?;
                    let verifier=brassclaw_auth::PkceVerifierSecret::new(authorization.code_verifier)?;
                    let expected_code=dispatch.request.authorization_code_hash.as_ref().ok_or(AuthProductError::BackendConflict)?;
                    let expected_verifier=dispatch.request.pkce_verifier_hash.as_ref().ok_or(AuthProductError::BackendConflict)?;
                    if !brassclaw_auth::authorization_code_hash(&code)?.constant_time_eq(expected_code)
                        || !brassclaw_auth::pkce_verifier_hash(&verifier)?.constant_time_eq(expected_verifier)
                        || dispatch.request.destinations.as_ref()!=Some(&original.definition.destinations) {
                        return Err(AuthProductError::BackendConflict);
                    }
                    Ok((format!("{auth}/oauth/token"),"application/x-www-form-urlencoded",
                        transport.native_device_exchange_body(&original.definition.client.client_id,
                            &format!("{auth}/deviceauth/callback"),code.expose_secret(),verifier.expose_secret())?))
                }
            }
        }.await;
        let (url, content_type, body) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => return self.record_native_device_unknown(&dispatch, error).await,
        };
        if let Err(error) = self.verify_native_device_dispatch(&dispatch).await {
            return self.record_native_device_unknown(&dispatch, error).await;
        }
        let request = match transport
            .prepare_oauth_http_request(
                original.definition.scope.resource.clone(),
                &url,
                content_type,
                body,
            )
            .await
        {
            Ok(request) => request,
            Err(error) => return self.record_native_device_unknown(&dispatch, error).await,
        };
        let custodian = NativeDeviceCustodian {
            auth: self,
            dispatch: &dispatch,
            expected: request.clone(),
            transport: None,
        };
        let returned = original_egress
            .execute_with_original_custody(request, Box::new(custodian))
            .await;
        // The normal response may be sanitized or fail after original custody.
        // Only the actual authenticated producer's record establishes a return.
        if let Some(known) = self.known_native_device_return(&dispatch).await? {
            return Ok(known);
        }
        let error = match returned {
            Err(_) => AuthProductError::BackendUnavailable,
            Ok(_) => AuthProductError::BackendConflict,
        };
        self.record_native_device_unknown(&dispatch, error).await
    }
}
impl PgAuthProductServices {
    async fn verify_native_device_dispatch(
        &self,
        dispatch: &NativeDeviceDispatch,
    ) -> Result<(), AuthProductError> {
        let original = &dispatch.original;
        let scope = &original.definition.scope;
        let mut client = self.pool.get().await.map_err(pool_err)?;
        let tx = client.transaction().await.map_err(db_err)?;
        lock_native_flow(&tx, scope, original.definition.flow_id).await?;
        let (document, _) =
            super::callback::locked_flow(&tx, scope, original.definition.flow_id).await?;
        let action = action(&document)?;
        if action.original != *original || action.requests.last() != Some(&dispatch.request) {
            return Err(AuthProductError::BackendConflict);
        }
        if Utc::now() >= action.expiry() {
            return Err(AuthProductError::UnknownOrExpiredFlow);
        }
        if document.flow.status != AuthFlowStatus::Completing {
            return Err(AuthProductError::FlowAlreadyTerminal);
        }
        tx.commit().await.map_err(db_err)?;
        Ok(())
    }
    /// Root must call this from its existing account/head/provider/role/receipt
    /// publication transaction. This checks the actual flow and raw SecretStore
    /// original, rather than trusting a copied ready/result descriptor.
    pub(crate) async fn native_device_ready_in_transaction(
        &self,
        tx: &tokio_postgres::Transaction<'_>,
        original: &NativeDeviceLoginOriginal,
    ) -> Result<NativeDeviceReady, AuthProductError> {
        self.native_device_ready_at(tx, original, Utc::now()).await
    }
    async fn native_device_ready_at(
        &self,
        tx: &tokio_postgres::Transaction<'_>,
        original: &NativeDeviceLoginOriginal,
        now: Timestamp,
    ) -> Result<NativeDeviceReady, AuthProductError> {
        lock_native_flow(tx, &original.definition.scope, original.definition.flow_id).await?;
        let (document, _) = super::callback::locked_flow(
            tx,
            &original.definition.scope,
            original.definition.flow_id,
        )
        .await?;
        let action = action(&document)?;
        if action.original != *original || document.flow.status != AuthFlowStatus::Completing {
            return Err(AuthProductError::BackendConflict);
        }
        self.authenticate_device_source(
            tx,
            original,
            &original.preparation,
            original.definition.preparation_context()?,
        )
        .await?;
        let request = action
            .requests
            .last()
            .ok_or(AuthProductError::BackendConflict)?;
        let NativeDeviceOutcome::Tokens {
            response,
            request_started_at,
            access_expires_at,
            ..
        } = &request.outcome
        else {
            return Err(AuthProductError::BackendConflict);
        };
        if *access_expires_at <= now
            || request.submitted_at >= action.expiry()
            || request.destinations.as_ref() != Some(&original.definition.destinations)
            || request.authorization_code_hash.is_none()
            || request.pkce_verifier_hash.is_none()
        {
            return Err(AuthProductError::BackendConflict);
        }
        let issued = transport_original(action, request)?;
        if issued.request_started_at >= action.expiry()
            || issued.request_started_at != *request_started_at
        {
            return Err(AuthProductError::BackendConflict);
        }
        self.authenticate_device_source(
            tx,
            original,
            &issued.source,
            request_context(original, &{
                let mut claim = request.clone();
                claim.outcome = NativeDeviceOutcome::Submitted;
                claim
            }),
        )
        .await?;
        let mut submitted = request.clone();
        submitted.outcome = NativeDeviceOutcome::Submitted;
        let actual = self
            .authenticate_device_source(
                tx,
                original,
                response,
                result_context(original, &submitted, transport_original(action, request)?),
            )
            .await?;
        Ok(NativeDeviceReady {
            original: original.clone(),
            request: request.clone(),
            response: source_from(&actual),
            access_expires_at: *access_expires_at,
            request_started_at: *request_started_at,
        })
    }
    /// One-shot raw input for Root's native publisher. Returned tokens have no
    /// serde; leases and account/head/token publication share Root's transaction.
    pub(crate) async fn native_device_tokens_in_transaction(
        &self,
        tx: &tokio_postgres::Transaction<'_>,
        ready: &NativeDeviceReady,
    ) -> Result<brassclaw_llm::CodexDeviceTokens, AuthProductError> {
        let actual = self
            .native_device_ready_in_transaction(tx, &ready.original)
            .await?;
        if actual.request != ready.request
            || actual.response != ready.response
            || actual.access_expires_at != ready.access_expires_at
            || actual.request_started_at != ready.request_started_at
        {
            return Err(AuthProductError::BackendConflict);
        }
        let material = self
            .read_native_device_original_in_transaction(tx, &ready.response)
            .await?;
        let raw = raw_from_material(&material)?;
        if !(200..300).contains(&raw.response.status) {
            return Err(AuthProductError::BackendConflict);
        }
        let NativeDeviceOutcome::Tokens {
            request_started_at, ..
        } = &ready.request.outcome
        else {
            return Err(AuthProductError::BackendConflict);
        };
        if raw.request_started_at != *request_started_at
            || raw.request_started_at != ready.request_started_at
        {
            return Err(AuthProductError::BackendConflict);
        }
        let tokens =
            brassclaw_llm::parse_codex_device_tokens(&raw.response.body, raw.request_started_at)
                .map_err(|_| AuthProductError::TokenExchangeFailed)?;
        if tokens.expires_at != ready.access_expires_at {
            return Err(AuthProductError::BackendConflict);
        }
        Ok(tokens)
    }
}
impl PgAuthProductServices {
    pub(crate) async fn native_device_challenge(
        &self,
        original: &NativeDeviceLoginOriginal,
    ) -> Result<Option<NativeDeviceChallenge>, AuthProductError> {
        let scope = &original.definition.scope;
        let id = original.definition.flow_id;
        let mut client = self.pool.get().await.map_err(pool_err)?;
        let tx = client.transaction().await.map_err(db_err)?;
        lock_native_flow(&tx, scope, id).await?;
        let (document, _) = super::callback::locked_flow(&tx, scope, id).await?;
        let action = action(&document)?;
        if action.original != *original {
            return Err(AuthProductError::BackendConflict);
        }
        if document.flow.status != AuthFlowStatus::AwaitingUser || Utc::now() >= action.expiry() {
            return Ok(None);
        }
        let request = action
            .requests
            .iter()
            .find(|r| matches!(r.outcome, NativeDeviceOutcome::Challenge { .. }))
            .ok_or(AuthProductError::BackendConflict)?;
        let NativeDeviceOutcome::Challenge { response, .. } = &request.outcome else {
            return Err(AuthProductError::BackendConflict);
        };
        let mut submitted = request.clone();
        submitted.outcome = NativeDeviceOutcome::Submitted;
        let actual = self
            .authenticate_device_source(
                &tx,
                original,
                response,
                result_context(original, &submitted, transport_original(action, request)?),
            )
            .await?;
        let material = self
            .read_native_device_original_in_transaction(&tx, &source_from(&actual))
            .await?;
        let raw = raw_from_material(&material)?;
        let parsed = brassclaw_llm::parse_codex_device_start(&raw.response.body, raw.received_at)
            .map_err(|_| AuthProductError::MalformedCallback)?;
        let (_, user_code, verification_uri, _, _) = parsed.into_native_parts();
        let challenge = NativeDeviceChallenge {
            user_code,
            verification_uri,
            expires_at: action.expiry(),
        };
        tx.commit().await.map_err(db_err)?;
        Ok(Some(challenge))
    }
}
#[cfg(all(test, feature = "skills-db"))]
#[path = "pg_auth_device_tests.rs"]
mod tests;

impl PgAuthProductServices {
    /// Finalize only the existing flow, as the LAST part of Root's existing
    /// account/head/provider/role/receipt transaction. Never commits or writes
    /// accounts, tokens, provider bindings, roles or mutation receipts itself.
    pub(crate) async fn finish_native_device_in_transaction(
        &self,
        tx: &tokio_postgres::Transaction<'_>,
        ready: &NativeDeviceReady,
        published: &CredentialAccount,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        self.finish_native_device_at(tx, ready, published, Utc::now())
            .await
    }
    async fn finish_native_device_at(
        &self,
        tx: &tokio_postgres::Transaction<'_>,
        ready: &NativeDeviceReady,
        published: &CredentialAccount,
        now: Timestamp,
    ) -> Result<AuthFlowRecord, AuthProductError> {
        let actual = self
            .native_device_ready_at(tx, &ready.original, now)
            .await?;
        if actual.request != ready.request
            || actual.response != ready.response
            || actual.access_expires_at != ready.access_expires_at
            || actual.request_started_at != ready.request_started_at
        {
            return Err(AuthProductError::BackendConflict);
        }
        let scope = &ready.original.definition.scope;
        let account = super::callback::locked_account(tx, scope, published.id)
            .await?
            .ok_or(AuthProductError::CredentialMissing)?;
        if account.0 != *published {
            return Err(AuthProductError::BackendConflict);
        }
        let (mut document, revision) =
            super::callback::locked_flow(tx, scope, ready.original.definition.flow_id).await?;
        let action = document
            .auth_custody
            .device_login
            .as_mut()
            .ok_or(AuthProductError::BackendConflict)?;
        action.finish(&mut document.flow, published, now)?;
        super::callback::save_flow(tx, &document, revision).await?;
        Ok(document.flow)
    }
}
