use crate::adapters::{TokioTimeProvider, WebPushSender};
use crate::config;
use crate::push_types::DirectiveRegistries;

mod directives;
mod registry;
mod scheduler;
mod vapid;

use std::sync::Arc;

use scheduler::PushScheduler;
pub(crate) use vapid::{VapidConfigStatus, load_vapid_config};

pub fn maybe_start_scheduler(config: &config::AppConfig, registries: Arc<DirectiveRegistries>) {
    let vapid = match load_vapid_config(config) {
        VapidConfigStatus::Ready(vapid) => vapid,
        VapidConfigStatus::Incomplete => {
            eprintln!("push notifications disabled: incomplete VAPID configuration");
            return;
        }
        VapidConfigStatus::Missing => {
            return;
        }
    };

    let sender = match WebPushSender::new(vapid) {
        Ok(sender) => sender,
        Err(err) => {
            eprintln!("push notifications disabled: failed to init web-push ({err})");
            return;
        }
    };

    let scheduler = PushScheduler::new(TokioTimeProvider, sender);
    scheduler.spawn_all(registries);
}
