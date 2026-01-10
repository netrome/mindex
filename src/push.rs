use crate::adapters::{TokioTimeProvider, WebPushSender};
use crate::config;
use crate::types::push::DirectiveRegistries;

mod directives;
mod registry;
mod scheduler;
pub(crate) mod vapid;

use std::sync::Arc;
use std::sync::Mutex;

use scheduler::PushScheduler;
pub(crate) use scheduler::ScheduledNotificationHandle;
pub(crate) use vapid::{VapidConfigStatus, load_vapid_config};

pub fn maybe_start_scheduler(
    config: &config::AppConfig,
    registries: Arc<DirectiveRegistries>,
    handles: Arc<Mutex<Vec<ScheduledNotificationHandle>>>,
) {
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
    let scheduled = scheduler.spawn_all(registries);
    let mut guard = handles.lock().expect("push handles lock");
    guard.extend(scheduled);
}

pub fn restart_scheduler(
    config: &config::AppConfig,
    registries: Arc<DirectiveRegistries>,
    handles: Arc<Mutex<Vec<ScheduledNotificationHandle>>>,
) {
    {
        let mut guard = handles.lock().expect("push handles lock");
        for handle in guard.drain(..) {
            handle.abort();
        }
    }
    maybe_start_scheduler(config, registries, handles);
}
