use crate::config::AppConfig;
use crate::push_types;

use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub push_registries: Arc<push_types::DirectiveRegistries>,
}
