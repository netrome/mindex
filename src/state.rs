use crate::config::AppConfig;
use crate::push;
use crate::push_types;

use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub push_registries: Arc<Mutex<push_types::DirectiveRegistries>>,
    pub push_handles: Arc<Mutex<Vec<push::ScheduledNotificationHandle>>>,
}
