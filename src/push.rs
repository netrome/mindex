use crate::adapters::{TokioTimeProvider, WebPushSender};
use crate::documents::{collect_markdown_paths, doc_id_from_path};
use crate::push_types::{DirectiveRegistries, Notification, VapidConfig};
use crate::{config, ports};

mod directives;

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use time::OffsetDateTime;
use tokio::task::JoinHandle;

impl DirectiveRegistries {
    pub fn load(root: &Path) -> std::io::Result<Self> {
        let mut registries = DirectiveRegistries::default();
        let paths = collect_markdown_paths(root)?;
        for path in paths {
            let doc_id = match doc_id_from_path(root, &path) {
                Some(doc_id) => doc_id,
                None => continue,
            };
            let contents = std::fs::read_to_string(&path)?;
            directives::parse_document(&doc_id, &contents, &mut registries);
        }
        Ok(registries)
    }
}

#[derive(Debug, Clone)]
pub struct PushScheduler<T, S> {
    time: T,
    sender: S,
}

impl<T, S> PushScheduler<T, S>
where
    T: ports::TimeProvider,
    S: ports::PushSender,
{
    pub fn new(time: T, sender: S) -> Self {
        Self { time, sender }
    }

    pub fn spawn_all(&self, registries: Arc<DirectiveRegistries>) -> Vec<JoinHandle<()>> {
        registries
            .notifications
            .iter()
            .map(|notification| {
                let time = self.time.clone();
                let sender = self.sender.clone();
                let registries = Arc::clone(&registries);
                let notification = notification.clone();
                tokio::spawn(async move {
                    run_notification(time, sender, registries, notification).await;
                })
            })
            .collect()
    }
}

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

fn compute_delay<T: ports::TimeProvider>(time: &T, at: OffsetDateTime) -> Option<Duration> {
    let now = time.now();
    let delay = at - now;
    if delay.is_positive() {
        match delay.try_into() {
            Ok(std_delay) => Some(std_delay),
            Err(_) => Some(Duration::MAX),
        }
    } else {
        None
    }
}

async fn run_notification<T, S>(
    time: T,
    sender: S,
    registries: Arc<DirectiveRegistries>,
    notification: Notification,
) where
    T: ports::TimeProvider,
    S: ports::PushSender,
{
    if let Some(delay) = compute_delay(&time, notification.at) {
        time.sleep(delay).await;
    }

    for recipient in &notification.to {
        let subscriptions = match registries.subscriptions.get(recipient) {
            Some(subscriptions) => subscriptions,
            None => {
                eprintln!(
                    "push delivery warning: no subscriptions for '{}' ({})",
                    recipient, notification.doc_id
                );
                continue;
            }
        };

        for subscription in subscriptions {
            if let Err(err) = sender.send(subscription, &notification.message).await {
                eprintln!(
                    "push delivery error: {} (user {}, doc {})",
                    err, recipient, notification.doc_id
                );
            }
        }
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;
    use crate::push_types::Subscription;
    use std::pin::Pin;
    use std::sync::Mutex;
    use std::task::{Context, Poll};
    use time::format_description::well_known::Rfc3339;
    use tokio::sync::oneshot;

    #[derive(Clone)]
    struct TestTime {
        now: OffsetDateTime,
        sleeps: Arc<Mutex<Vec<oneshot::Sender<()>>>>,
        durations: Arc<Mutex<Vec<Duration>>>,
    }

    impl TestTime {
        fn new(now: OffsetDateTime) -> Self {
            Self {
                now,
                sleeps: Arc::new(Mutex::new(Vec::new())),
                durations: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn sleep_durations(&self) -> Vec<Duration> {
            self.durations.lock().expect("durations lock").clone()
        }

        fn trigger_all(&self) {
            let mut sends = self.sleeps.lock().expect("sleeps lock");
            for sender in sends.drain(..) {
                let _ = sender.send(());
            }
        }
    }

    struct ManualSleep {
        receiver: oneshot::Receiver<()>,
    }

    impl Future for ManualSleep {
        type Output = ();

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            match Pin::new(&mut self.receiver).poll(cx) {
                Poll::Ready(_) => Poll::Ready(()),
                Poll::Pending => Poll::Pending,
            }
        }
    }

    impl ports::TimeProvider for TestTime {
        type Sleep<'a>
            = ManualSleep
        where
            Self: 'a;

        fn now(&self) -> OffsetDateTime {
            self.now
        }

        fn sleep<'a>(&'a self, duration: Duration) -> Self::Sleep<'a> {
            let (sender, receiver) = oneshot::channel();
            self.durations
                .lock()
                .expect("durations lock")
                .push(duration);
            self.sleeps.lock().expect("sleeps lock").push(sender);
            ManualSleep { receiver }
        }
    }

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

    impl ports::PushSender for TestSender {
        type Error = TestSendError;
        type Fut<'a>
            = std::future::Ready<Result<(), Self::Error>>
        where
            Self: 'a;

        fn send<'a>(&'a self, subscription: &'a Subscription, message: &'a str) -> Self::Fut<'a> {
            self.sent
                .lock()
                .expect("sent lock")
                .push((subscription.endpoint.clone(), message.to_string()));
            std::future::ready(Ok(()))
        }
    }

    #[test]
    fn compute_delay__should_return_none_for_past() {
        let now = OffsetDateTime::parse("2025-01-12T09:30:00Z", &Rfc3339).expect("parse now");
        let time = TestTime::new(now);
        let at = now - time::Duration::seconds(5);

        assert!(compute_delay(&time, at).is_none());
    }

    #[test]
    fn compute_delay__should_return_none_for_now() {
        let now = OffsetDateTime::parse("2025-01-12T09:30:00Z", &Rfc3339).expect("parse now");
        let time = TestTime::new(now);

        assert!(compute_delay(&time, now).is_none());
    }

    #[test]
    fn compute_delay__should_return_duration_for_future() {
        let now = OffsetDateTime::parse("2025-01-12T09:30:00Z", &Rfc3339).expect("parse now");
        let time = TestTime::new(now);
        let at = now + time::Duration::milliseconds(1500);

        let delay = compute_delay(&time, at).expect("delay");
        assert_eq!(delay, Duration::from_millis(1500));
    }

    #[tokio::test]
    async fn scheduler__should_wait_and_send() {
        let now = OffsetDateTime::parse("2025-01-12T09:30:00Z", &Rfc3339).expect("parse now");
        let time = TestTime::new(now);
        let sender = TestSender::default();
        let notification = Notification {
            to: vec!["marten".to_string()],
            at: now + time::Duration::seconds(30),
            message: "Hello".to_string(),
            doc_id: "note.md".to_string(),
        };
        let mut registries = DirectiveRegistries::default();
        registries.subscriptions.insert(
            "marten".to_string(),
            vec![Subscription {
                endpoint: "https://push.example/123".to_string(),
                p256dh: "p256".to_string(),
                auth: "auth".to_string(),
            }],
        );
        registries.notifications.push(notification);

        let scheduler = PushScheduler::new(time.clone(), sender.clone());
        let handles = scheduler.spawn_all(Arc::new(registries));

        tokio::task::yield_now().await;
        assert_eq!(sender.sent.lock().expect("sent lock").len(), 0);
        assert_eq!(time.sleep_durations(), vec![Duration::from_secs(30)]);

        time.trigger_all();
        for handle in handles {
            handle.await.expect("join handle");
        }

        let sent = sender.sent.lock().expect("sent lock");
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].0, "https://push.example/123");
        assert_eq!(sent[0].1, "Hello");
    }

    #[tokio::test]
    async fn scheduler__should_send_immediately_for_past_notification() {
        let now = OffsetDateTime::parse("2025-01-12T09:30:00Z", &Rfc3339).expect("parse now");
        let time = TestTime::new(now);
        let sender = TestSender::default();
        let notification = Notification {
            to: vec!["marten".to_string()],
            at: now - time::Duration::seconds(5),
            message: "Immediate".to_string(),
            doc_id: "note.md".to_string(),
        };
        let mut registries = DirectiveRegistries::default();
        registries.subscriptions.insert(
            "marten".to_string(),
            vec![Subscription {
                endpoint: "https://push.example/123".to_string(),
                p256dh: "p256".to_string(),
                auth: "auth".to_string(),
            }],
        );
        registries.notifications.push(notification);

        let scheduler = PushScheduler::new(time.clone(), sender.clone());
        let handles = scheduler.spawn_all(Arc::new(registries));

        for handle in handles {
            handle.await.expect("join handle");
        }

        assert!(time.sleep_durations().is_empty());
        let sent = sender.sent.lock().expect("sent lock");
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].1, "Immediate");
    }
}
