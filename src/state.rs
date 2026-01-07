use crate::config::AppConfig;
use crate::push;

use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub push_registries: Arc<push::DirectiveRegistries>,
}
