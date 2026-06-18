use std::sync::Arc;

use crate::AppEvent;

pub trait EventPublisher: Send + Sync {
    fn broadcast(&self, event: AppEvent);
    fn broadcast_for_user(&self, user_id: &str, event: AppEvent);
    fn has_receivers(&self) -> bool;
    fn has_verbose_receivers(&self) -> bool;
}

pub type DynEventPublisher = Arc<dyn EventPublisher>;
