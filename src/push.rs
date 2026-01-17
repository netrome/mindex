use crate::adapters::{TokioTimeProvider, WebPushSender};
use crate::config;
use crate::ports::push::PushSender;
use crate::types::directives::DirectiveRegistries;

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

pub async fn send_mentions(
    config: &config::AppConfig,
    registries: &DirectiveRegistries,
    doc_id: &str,
    mentions: &[(String, String)],
) {
    if mentions.is_empty() {
        return;
    }

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

    send_mentions_with_sender(sender, registries, doc_id, mentions).await;
}

async fn send_mentions_with_sender<S: PushSender>(
    sender: S,
    registries: &DirectiveRegistries,
    doc_id: &str,
    mentions: &[(String, String)],
) {
    for (recipient, message) in mentions {
        let subscriptions = match registries.subscriptions.get(recipient) {
            Some(subscriptions) => subscriptions,
            None => {
                eprintln!(
                    "push delivery warning: no subscriptions for '{}' ({})",
                    recipient, doc_id
                );
                continue;
            }
        };

        for subscription in subscriptions {
            if let Err(err) = sender.send(subscription, message).await {
                eprintln!(
                    "push delivery error: {} (user {}, doc {})",
                    err, recipient, doc_id
                );
            }
        }
    }
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

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;
    use crate::types::directives::Subscription;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Arc;
    use std::sync::Mutex;

    #[derive(Debug)]
    struct TestSendError;

    impl std::fmt::Display for TestSendError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("test send error")
        }
    }

    #[derive(Clone, Default)]
    struct TestSender {
        sent: Arc<Mutex<Vec<(String, String)>>>,
    }

    impl PushSender for TestSender {
        type Error = TestSendError;
        type Fut<'a>
            = Pin<Box<dyn Future<Output = Result<(), Self::Error>> + Send + 'a>>
        where
            Self: 'a;

        fn send<'a>(&'a self, subscription: &'a Subscription, message: &'a str) -> Self::Fut<'a> {
            let sent = Arc::clone(&self.sent);
            let endpoint = subscription.endpoint.clone();
            let message = message.to_string();
            Box::pin(async move {
                sent.lock().expect("sent lock").push((endpoint, message));
                Ok(())
            })
        }
    }

    #[tokio::test]
    async fn send_mentions_with_sender__should_send_for_each_line() {
        // Given
        let mut registries = DirectiveRegistries::default();
        registries.subscriptions.insert(
            "marten".to_string(),
            vec![Subscription {
                endpoint: "https://push.example/123".to_string(),
                p256dh: "p256".to_string(),
                auth: "auth".to_string(),
            }],
        );
        let mentions = vec![
            ("marten".to_string(), "First line".to_string()),
            ("marten".to_string(), "Second line".to_string()),
        ];
        let sender = TestSender::default();

        // When
        send_mentions_with_sender(sender.clone(), &registries, "note.md", &mentions).await;

        // Then
        let sent = sender.sent.lock().expect("sent lock").clone();
        assert_eq!(sent.len(), 2);
        assert_eq!(sent[0].0, "https://push.example/123");
        assert_eq!(sent[0].1, "First line");
        assert_eq!(sent[1].1, "Second line");
    }
}
