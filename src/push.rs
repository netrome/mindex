use crate::adapters::{TokioTimeProvider, WebPushSender};
use crate::config;
use crate::push_types::{DirectiveRegistries, VapidConfig};

mod directives;
mod registry;
mod scheduler;

use std::sync::Arc;

use scheduler::PushScheduler;

pub fn maybe_start_scheduler(config: &config::AppConfig, registries: Arc<DirectiveRegistries>) {
    let has_any = config.vapid_private_key.is_some()
        || config.vapid_public_key.is_some()
        || config.vapid_subject.is_some();
    let vapid = match (
        config.vapid_private_key.as_ref(),
        config.vapid_public_key.as_ref(),
        config.vapid_subject.as_ref(),
    ) {
        (Some(private_key), Some(public_key), Some(subject)) => Some(VapidConfig {
            private_key: private_key.clone(),
            public_key: public_key.clone(),
            subject: subject.clone(),
        }),
        _ => None,
    };

    let vapid = match vapid {
        Some(vapid) => vapid,
        None => {
            if has_any {
                eprintln!("push notifications disabled: incomplete VAPID configuration");
            }
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
