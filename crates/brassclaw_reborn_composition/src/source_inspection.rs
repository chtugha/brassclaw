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

struct Bound {
    settings: Arc<dyn HostPrimitiveSettingsSource>,
    worker: Arc<PackagedExecutableImage>,
}

pub(crate) struct ContainedSourceInspection {
    bound: RwLock<Option<Bound>>,
}
fn error(kind: RuntimeDispatchErrorKind) -> FirstPartyCapabilityError {
    FirstPartyCapabilityError::new(kind)
}
impl ContainedSourceInspection {
    pub(crate) fn new() -> Self {
        Self {
            bound: RwLock::new(None),
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
        let PreparedUtility {
            worker,
            limits,
            revision,
        } = self.prepare().await?;
        if source.len() > limits.bounds.max_source_bytes {
            return Err(error(RuntimeDispatchErrorKind::Resource));
        }
        let started = Instant::now();
        let inspected = utility::execute(
            &worker.path(),
            UtilityRequest::InspectSource {
                source: source.to_owned(),
                bounds: limits.bounds,
            },
            limits.process,
        )
        .await;
        let usage = ResourceUsage {
            wall_clock_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            process_count: 1,
            ..ResourceUsage::default()
        };
        let UtilityOutput::Inspected { structure } = inspected.map_err(|failure| {
            // Private parser exception text may contain candidate source. Keep
            // it out of runtime errors; no result is manufactured after failure.
            let kind = match failure.kind {
                brassclaw_monty_host::process::ProcessFailure::ValueLimit
                | brassclaw_monty_host::process::ProcessFailure::FrameLimit
                | brassclaw_monty_host::process::ProcessFailure::Vm(
                    brassclaw_monty_host::VmFailure::ResourceLimit
                    | brassclaw_monty_host::VmFailure::SourceLimit,
                ) => RuntimeDispatchErrorKind::Resource,
                brassclaw_monty_host::process::ProcessFailure::Vm(
                    brassclaw_monty_host::VmFailure::Python,
                ) => RuntimeDispatchErrorKind::InputEncode,
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
        })?
        else {
            return Err(error(RuntimeDispatchErrorKind::OperationFailed).with_usage(usage));
        };
        let output = json!({"format":"python-source-facts/1", "preparation_revision":revision,
            "worker_checksum":hex::encode(worker.checksum()),"facts":structure});
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

pub(crate) struct PreparedUtility {
    pub(crate) worker: Arc<PackagedExecutableImage>,
    pub(crate) limits: UtilityLimits,
    pub(crate) revision: u64,
}
impl ContainedSourceInspection {
    /// Share one retained worker/settings owner with fixed preparation
    /// primitives. Bootstrap selection is private; ingress sees live ack limits.
    pub(crate) async fn prepare(&self) -> Result<PreparedUtility, FirstPartyCapabilityError> {
        let (settings, worker) = {
            let bound = self
                .bound
                .read()
                .map_err(|_| error(RuntimeDispatchErrorKind::OperationFailed))?;
            let bound = bound
                .as_ref()
                .ok_or_else(|| error(RuntimeDispatchErrorKind::OperationFailed))?;
            (bound.settings.clone(), bound.worker.clone())
        };
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
