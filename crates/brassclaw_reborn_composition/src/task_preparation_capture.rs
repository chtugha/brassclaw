//! Application-owned capture from acknowledged settings. The neutral codec
//! validates original bytes; this facade also checks the native worker profile.
pub(crate) use brassclaw_host_api::task_preparation::PreparationOrigin;
use brassclaw_host_api::{MontyExecutionLimits, task_preparation::RetainedPreparation};
use brassclaw_monty_host::utility::UtilityLimits;

#[derive(Clone)]
pub(crate) struct PreparationCapture {
    original: RetainedPreparation,
    limits: UtilityLimits,
}
impl PreparationCapture {
    pub(super) fn capture(
        settings: &brassclaw_product_workflow::MontyVmSettings,
        origin: PreparationOrigin,
    ) -> Result<Self, &'static str> {
        let limits = UtilityLimits::from_execution(settings.execution_limits)?;
        let original =
            RetainedPreparation::encode(settings.revision, origin, settings.execution_limits)?;
        Ok(Self { original, limits })
    }
    pub(crate) fn restore_original(
        bytes: &str,
        expected_checksum: &str,
    ) -> Result<Self, &'static str> {
        Self::from_original(RetainedPreparation::restore_original(
            bytes,
            expected_checksum,
        )?)
    }
    pub(crate) fn from_original(original: RetainedPreparation) -> Result<Self, &'static str> {
        let limits = UtilityLimits::from_execution(original.policy())?;
        Ok(Self { original, limits })
    }
    pub(crate) fn original(&self) -> &RetainedPreparation {
        &self.original
    }
    pub(crate) fn revision(&self) -> u64 {
        self.original.revision()
    }
    pub(crate) fn policy(&self) -> MontyExecutionLimits {
        self.original.policy()
    }
    pub(crate) fn limits(&self) -> UtilityLimits {
        self.limits
    }
    pub(crate) fn bytes(&self) -> &str {
        self.original.bytes()
    }
    pub(crate) fn checksum(&self) -> &str {
        self.original.checksum()
    }
}
#[cfg(all(
    test,
    feature = "postgres",
    feature = "skills-db",
    feature = "test-support"
))]
#[path = "task_preparation_capture_tests.rs"]
mod tests;
