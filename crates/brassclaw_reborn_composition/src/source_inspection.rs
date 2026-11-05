//! Instance-owned mechanical source facts. No class-specific validation policy,
//! candidate execution, workflow sequencing, approval, or model calls live here.
use std::{
    sync::{Arc, RwLock},
    time::Instant,
};

use async_trait::async_trait;
use brassclaw_host_api::{ResourceUsage, RuntimeDispatchErrorKind};
use brassclaw_host_runtime::{
    FirstPartyCapabilityError, FirstPartyCapabilityResult, HostPrimitiveSettingsSource,
    PackagedExecutableImage, SourceInspectionBackend,
};
use brassclaw_monty_host::{
    process::installed_worker,
    utility::{self, UtilityLimits, UtilityOutput, UtilityRequest},
};
use serde_json::json;

#[derive(Clone)]
struct Bound {
    settings: Arc<dyn HostPrimitiveSettingsSource>,
    worker: Arc<PackagedExecutableImage>,
}

pub(crate) struct ContainedSourceInspection {
    bound: RwLock<Option<Bound>>,
    binding_id: uuid::Uuid,
}
fn error(kind: RuntimeDispatchErrorKind) -> FirstPartyCapabilityError {
    FirstPartyCapabilityError::new(kind)
}
impl ContainedSourceInspection {
    pub(crate) fn new() -> Self {
        Self {
            bound: RwLock::new(None),
            binding_id: uuid::Uuid::new_v4(),
        }
    }

    /// Bind the installation-owned settings source before qualification. It
    /// supplies the exact selected bootstrap limits, then transitions to the
    /// acknowledged live view before ingress. Candidate data chooses neither.
    pub(crate) async fn bind(
        &self,
        settings: Arc<dyn HostPrimitiveSettingsSource>,
    ) -> Result<(), crate::RebornBuildError> {
        let fail = || crate::RebornBuildError::InvalidConfig {
            reason: "source inspection initialization failed".into(),
        };
        if self.bound.read().map_err(|_| fail())?.is_some() {
            return Err(fail());
        }
        let current = settings.snapshot().await.map_err(|_| fail())?;
        let limits = UtilityLimits::from_execution(current.limits).map_err(|_| fail())?;
        let worker = Arc::new(
            PackagedExecutableImage::capture(
                &installed_worker().map_err(|_| fail())?,
                limits.max_native_artifact_bytes,
            )
            .await
            .map_err(|_| fail())?,
        );
        // A contained fixed input qualifies protocol/parse facts without running
        // candidate code, or relying on this Tool to approve its own component.
        let source = "def inspection_probe(inputs):\n    return inputs\n";
        let UtilityOutput::Inspected { structure } = utility::execute(
            &worker.path(),
            UtilityRequest::InspectSource {
                source: source.into(),
                bounds: limits.bounds,
            },
            limits.process,
        )
        .await
        .map_err(|_| fail())?
        else {
            return Err(fail());
        };
        use sha2::Digest;
        if structure.source_checksum != hex::encode(sha2::Sha256::digest(source.as_bytes()))
            || !structure.direct_host_calls.is_empty()
            || !structure.imports.is_empty()
            || structure.preload.as_ref().is_none_or(|preload| {
                preload.functions.get("inspection_probe") != Some(&vec!["inputs".to_owned()])
            })
        {
            return Err(fail());
        }
        let mut bound = self.bound.write().map_err(|_| fail())?;
        if bound.is_some() {
            return Err(fail());
        }
        *bound = Some(Bound { settings, worker });
        Ok(())
    }
}

#[async_trait]
impl SourceInspectionBackend for ContainedSourceInspection {
    async fn inspect(
        &self,
        source: &str,
    ) -> Result<FirstPartyCapabilityResult, FirstPartyCapabilityError> {
        let prepared = self.prepare().await?;
        if source.len() > prepared.limits.bounds.max_source_bytes {
            return Err(error(RuntimeDispatchErrorKind::Resource));
        }
        let started = Instant::now();
        let (output, usage) = prepared
            .execute(
                UtilityRequest::InspectSource {
                    source: source.to_owned(),
                    bounds: prepared.limits.bounds,
                },
                started,
            )
            .await?;
        let UtilityOutput::Inspected { structure } = output else {
            return Err(error(RuntimeDispatchErrorKind::OperationFailed).with_usage(usage));
        };
        prepared.report(
            json!({"format":"python-source-facts/1", "preparation_revision":prepared.revision,
            "worker_checksum":hex::encode(prepared.worker.checksum()),"facts":structure}),
            usage,
        )
    }

    async fn inspect_sources(
        &self,
        sources: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<FirstPartyCapabilityResult, FirstPartyCapabilityError> {
        self.prepare().await?.inspect_sources(sources, None).await
    }
    async fn capture_fixture_preparation(
        &self,
    ) -> Result<brassclaw_host_runtime::SourceInspectionPreparationFacts, FirstPartyCapabilityError>
    {
        let Bound { settings, worker } = self.bound_parts()?;
        let capture = settings.capture_for_native_fixture().await?;
        Ok(brassclaw_host_runtime::SourceInspectionPreparationFacts {
            capture,
            worker_checksum: worker.checksum(),
            backend_binding_id: self.binding_id,
        })
    }
    async fn inspect_sources_with_fixture(
        &self,
        sources: &serde_json::Map<String, serde_json::Value>,
        fixture: &brassclaw_host_runtime::SourceInspectionFixtureProfile,
    ) -> Result<FirstPartyCapabilityResult, FirstPartyCapabilityError> {
        let actual = self.capture_fixture_preparation().await?;
        if actual != fixture.base() || fixture.arguments() != &json!({"sources":sources}) {
            return Err(error(RuntimeDispatchErrorKind::OperationFailed));
        }
        let Bound { worker, .. } = self.bound_parts()?;
        let limits = UtilityLimits::from_execution(fixture.limits())
            .map_err(|_| error(RuntimeDispatchErrorKind::Resource))?;
        PreparedUtility {
            worker,
            limits,
            revision: actual.capture.settings.revision,
        }
        .inspect_sources(sources, Some(fixture.guard_reason()))
        .await
    }
}

pub(crate) struct PreparedUtility {
    pub(crate) worker: Arc<PackagedExecutableImage>,
    pub(crate) limits: UtilityLimits,
    pub(crate) revision: u64,
}
impl PreparedUtility {
    async fn inspect_sources(
        &self,
        sources: &serde_json::Map<String, serde_json::Value>,
        fixture_reason: Option<String>,
    ) -> Result<FirstPartyCapabilityResult, FirstPartyCapabilityError> {
        if sources
            .iter()
            .any(|(id, source)| id.is_empty() || !source.is_string())
        {
            return Err(error(RuntimeDispatchErrorKind::InputEncode));
        }
        // This stable native guard diagnostic identifies the pre-worker source
        // byte rejection. Generic Resource failures (allocator/transport/worker)
        // are deliberately different and cannot qualify this boundary case.
        let source_bytes = sources
            .values()
            .try_fold(0usize, |total, source| {
                source
                    .as_str()
                    .and_then(|source| total.checked_add(source.len()))
            })
            .ok_or_else(|| error(RuntimeDispatchErrorKind::Resource))?;
        if source_bytes > self.limits.bounds.max_source_bytes {
            return Err(FirstPartyCapabilityError::with_safe_summary(
                RuntimeDispatchErrorKind::Resource,
                fixture_reason.unwrap_or_else(|| format!(
                    "source inventory source byte limit exceeded revision {} limit {} actual {}",
                    self.revision, self.limits.bounds.max_source_bytes, source_bytes
                )),
            ).with_usage(ResourceUsage::default()));
        }
        if !brassclaw_monty_host::source_inventory::source_values_within_bounds(
            sources,
            self.limits.bounds,
            self.limits.process.max_frame_bytes,
        ) {
            return Err(error(RuntimeDispatchErrorKind::Resource));
        }
        let started = Instant::now();
        let sources = sources
            .iter()
            .map(|(id, value)| {
                value
                    .as_str()
                    .map(|source| (id.clone(), source.to_owned()))
                    .ok_or_else(|| error(RuntimeDispatchErrorKind::InputEncode))
            })
            .collect::<Result<_, _>>()?;
        let (output, usage) = self
            .execute(
                UtilityRequest::InspectSources {
                    sources,
                    bounds: self.limits.bounds,
                },
                started,
            )
            .await?;
        let UtilityOutput::InspectedSources { sources } = output else {
            return Err(error(RuntimeDispatchErrorKind::OperationFailed).with_usage(usage));
        };
        self.report(
            json!({"format":"python-source-inventory/1", "preparation_revision":self.revision,
            "worker_checksum":hex::encode(self.worker.checksum()),"sources":sources}),
            usage,
        )
    }

    async fn execute(
        &self,
        request: UtilityRequest,
        started: Instant,
    ) -> Result<(UtilityOutput, ResourceUsage), FirstPartyCapabilityError> {
        let single_source = matches!(&request, UtilityRequest::InspectSource { .. });
        let inspected = utility::execute(&self.worker.path(), request, self.limits.process).await;
        let usage = ResourceUsage {
            wall_clock_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            process_count: 1,
            ..ResourceUsage::default()
        };
        let output = inspected.map_err(|failure| {
            // Private Ausnahmen können Quelltext enthalten. Ein Infrastruktur-
            // fehler wird niemals zum negativen Ergebnis einer einzelnen Quelle.
            let kind = match failure.kind {
                brassclaw_monty_host::process::ProcessFailure::ValueLimit
                | brassclaw_monty_host::process::ProcessFailure::FrameLimit
                | brassclaw_monty_host::process::ProcessFailure::Vm(
                    brassclaw_monty_host::VmFailure::ResourceLimit
                    | brassclaw_monty_host::VmFailure::SourceLimit,
                ) => RuntimeDispatchErrorKind::Resource,
                brassclaw_monty_host::process::ProcessFailure::Vm(
                    brassclaw_monty_host::VmFailure::Python,
                ) if single_source => RuntimeDispatchErrorKind::InputEncode,
                _ => RuntimeDispatchErrorKind::OperationFailed,
            };
            let mut failed_usage = usage.clone();
            if matches!(
                failure.kind,
                brassclaw_monty_host::process::ProcessFailure::InvalidLimits
                    | brassclaw_monty_host::process::ProcessFailure::InvalidExecutable
                    | brassclaw_monty_host::process::ProcessFailure::FrameLimit
                    | brassclaw_monty_host::process::ProcessFailure::ValueLimit
                    | brassclaw_monty_host::process::ProcessFailure::Spawn
            ) {
                failed_usage.process_count = 0;
            }
            error(kind).with_usage(failed_usage)
        })?;
        Ok((output, usage))
    }

    fn report(
        &self,
        output: serde_json::Value,
        usage: ResourceUsage,
    ) -> Result<FirstPartyCapabilityResult, FirstPartyCapabilityError> {
        let limits = self.limits;
        let bounds = brassclaw_skills::value_contract::ContractLimits {
            max_depth: limits.bounds.max_value_depth,
            max_nodes: limits.bounds.max_value_nodes,
            max_bytes: limits.bounds.max_value_bytes,
        };
        brassclaw_skills::value_contract::validate_data_bounds(&output, bounds)
            .map_err(|_| error(RuntimeDispatchErrorKind::Resource).with_usage(usage.clone()))?;
        let bytes = serde_json::to_string(&output).map_err(|_| {
            error(RuntimeDispatchErrorKind::OperationFailed).with_usage(usage.clone())
        })?;
        if bytes.len() > limits.bounds.max_value_bytes {
            return Err(error(RuntimeDispatchErrorKind::Resource).with_usage(usage));
        }
        let value = json!(bytes);
        let encoded = serde_json::to_vec(&value).map_err(|_| {
            error(RuntimeDispatchErrorKind::OperationFailed).with_usage(usage.clone())
        })?;
        if encoded.len() > limits.bounds.max_value_bytes
            || encoded.len() > limits.process.max_frame_bytes
        {
            return Err(error(RuntimeDispatchErrorKind::Resource).with_usage(usage));
        }
        let mut usage = usage;
        usage.output_bytes = encoded.len() as u64;
        Ok(FirstPartyCapabilityResult::new(value, usage))
    }
}

impl ContainedSourceInspection {
    /// Share one retained worker/settings owner with fixed preparation
    /// primitives. Bootstrap selection is private; ingress sees live ack limits.
    fn bound_parts(&self) -> Result<Bound, FirstPartyCapabilityError> {
        self.bound
            .read()
            .map_err(|_| error(RuntimeDispatchErrorKind::OperationFailed))?
            .clone()
            .ok_or_else(|| error(RuntimeDispatchErrorKind::OperationFailed))
    }
    pub(crate) async fn prepare(&self) -> Result<PreparedUtility, FirstPartyCapabilityError> {
        let Bound { settings, worker } = self.bound_parts()?;
        let current = settings
            .snapshot()
            .await
            .map_err(|_| error(RuntimeDispatchErrorKind::OperationFailed))?;
        let limits = UtilityLimits::from_execution(current.limits)
            .map_err(|_| error(RuntimeDispatchErrorKind::Resource))?;
        Ok(PreparedUtility {
            worker,
            limits,
            revision: current.revision,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn source_bootstrap_preparation_uses_the_selected_shared_settings_owner() {
        let source = ContainedSourceInspection::new();
        assert!(source.prepare().await.is_err());
        let settings =
            Arc::new(crate::host_primitive_settings::AcknowledgedPrimitiveSettings::new());
        let limits = brassclaw_host_api::MontyExecutionLimits {
            max_source_bytes: 4096,
            max_compiled_source_bytes: 8192,
            ..brassclaw_host_api::MontyExecutionLimits::default()
        };
        settings
            .prepare_bootstrap(brassclaw_host_runtime::HostPrimitiveSettings {
                revision: 7,
                limits,
            })
            .unwrap();
        source.bind(settings.clone()).await.unwrap();
        let prepared = source.prepare().await.unwrap();
        assert_eq!(prepared.revision, 7);
        assert_eq!(prepared.limits.bounds.max_source_bytes, 4096);
        assert_eq!(prepared.limits.bounds.max_compiled_source_bytes, 8192);
        let identity = prepared.worker.checksum();
        assert!(
            source.bind(settings).await.is_err(),
            "a retained preparation worker cannot be replaced"
        );
        assert_eq!(source.prepare().await.unwrap().worker.checksum(), identity);
    }
}
