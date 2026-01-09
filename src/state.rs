use crate::config::AppConfig;
use crate::push as push_service;
use crate::types::push;

use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub push_registries: Arc<Mutex<push::DirectiveRegistries>>,
    pub push_handles: Arc<Mutex<Vec<push_service::ScheduledNotificationHandle>>>,
}
