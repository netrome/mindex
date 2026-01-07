use crate::adapters::{TokioTimeProvider, WebPushSender};
use crate::app::{collect_markdown_paths, doc_id_from_path};
use crate::{config, ports};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tokio::task::JoinHandle;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DirectiveRegistries {
    pub users: HashMap<String, User>,
    pub subscriptions: HashMap<String, Vec<Subscription>>,
    pub notifications: Vec<Notification>,
}

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
            parse_document(&doc_id, &contents, &mut registries);
        }
        Ok(registries)
    }
}

#[derive(Debug, Clone)]
pub struct VapidConfig {
    pub private_key: String,
    #[allow(unused)] // TODO: Will be used when we implement subscription UI
    pub public_key: String,
    pub subject: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub name: String,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub to: Vec<String>,
    pub at: OffsetDateTime,
    pub message: String,
    pub doc_id: String,
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
        let std_delay: Duration = delay.try_into().unwrap_or_default();
        Some(std_delay)
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

#[derive(Debug, Clone, Copy)]
enum DirectiveKind {
    User,
    Subscription,
    Notify,
}

impl DirectiveKind {
    fn label(self) -> &'static str {
        match self {
            DirectiveKind::User => "/user",
            DirectiveKind::Subscription => "/subscription",
            DirectiveKind::Notify => "/notify",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct PendingDirective {
    kind: DirectiveKind,
    line: usize,
}

#[derive(Debug)]
struct FenceLine {
    ticks: usize,
    language: Option<String>,
}

fn parse_document(doc_id: &str, contents: &str, registries: &mut DirectiveRegistries) {
    let lines: Vec<&str> = contents.lines().collect();
    let mut idx = 0usize;
    let mut pending: Option<PendingDirective> = None;

    while idx < lines.len() {
        let line = lines[idx];
        let trimmed = line.trim();

        if let Some(kind) = parse_directive_line(trimmed) {
            if let Some(previous) = pending.take() {
                warn(
                    doc_id,
                    previous.line,
                    format!("missing toml block after {}", previous.kind.label()),
                );
            }
            pending = Some(PendingDirective {
                kind,
                line: idx + 1,
            });
            idx += 1;
            continue;
        }

        if let Some(fence) = parse_fence_line(trimmed) {
            let fence_line = idx + 1;
            idx += 1;
            let mut block_lines = Vec::new();
            let mut closed = false;

            while idx < lines.len() {
                let line = lines[idx];
                if is_fence_close(line, fence.ticks) {
                    closed = true;
                    idx += 1;
                    break;
                }
                block_lines.push(line);
                idx += 1;
            }

            if let Some(directive) = pending.take() {
                if !closed {
                    warn(
                        doc_id,
                        directive.line,
                        format!("unterminated toml block after {}", directive.kind.label()),
                    );
                } else if !is_toml_language(fence.language.as_deref()) {
                    warn(
                        doc_id,
                        directive.line,
                        format!("expected toml block after {}", directive.kind.label()),
                    );
                } else {
                    let toml_text = block_lines.join("\n");
                    handle_directive_block(doc_id, fence_line, directive, &toml_text, registries);
                }
            }
            continue;
        }

        idx += 1;
    }

    if let Some(directive) = pending {
        warn(
            doc_id,
            directive.line,
            format!("missing toml block after {}", directive.kind.label()),
        );
    }
}

fn parse_directive_line(line: &str) -> Option<DirectiveKind> {
    match line {
        "/user" => Some(DirectiveKind::User),
        "/subscription" => Some(DirectiveKind::Subscription),
        "/notify" => Some(DirectiveKind::Notify),
        _ => None,
    }
}

fn parse_fence_line(line: &str) -> Option<FenceLine> {
    let trimmed = line.trim();
    let mut chars = trimmed.chars();
    let mut ticks = 0usize;
    while matches!(chars.next(), Some('`')) {
        ticks += 1;
    }
    if ticks < 3 {
        return None;
    }
    let rest = trimmed[ticks..].trim();
    let language = if rest.is_empty() {
        None
    } else {
        Some(rest.split_whitespace().next().unwrap_or("").to_string())
    };
    Some(FenceLine { ticks, language })
}

fn is_fence_close(line: &str, ticks: usize) -> bool {
    let trimmed = line.trim();
    let mut chars = trimmed.chars();
    for _ in 0..ticks {
        if chars.next() != Some('`') {
            return false;
        }
    }
    chars.all(|ch| ch.is_whitespace())
}

fn is_toml_language(language: Option<&str>) -> bool {
    matches!(language, Some(lang) if lang.eq_ignore_ascii_case("toml"))
}

fn handle_directive_block(
    doc_id: &str,
    block_line: usize,
    directive: PendingDirective,
    toml_text: &str,
    registries: &mut DirectiveRegistries,
) {
    match directive.kind {
        DirectiveKind::User => parse_user_block(doc_id, block_line, toml_text, registries),
        DirectiveKind::Subscription => {
            parse_subscription_block(doc_id, block_line, toml_text, registries)
        }
        DirectiveKind::Notify => parse_notify_block(doc_id, block_line, toml_text, registries),
    }
}

#[derive(Debug, Deserialize)]
struct UserToml {
    name: String,
    display_name: Option<String>,
}

fn parse_user_block(
    doc_id: &str,
    block_line: usize,
    toml_text: &str,
    registries: &mut DirectiveRegistries,
) {
    let parsed: UserToml = match toml::from_str(toml_text) {
        Ok(parsed) => parsed,
        Err(err) => {
            warn(
                doc_id,
                block_line,
                format!("invalid toml for /user block: {err}"),
            );
            return;
        }
    };

    let name = parsed.name.trim();
    if name.is_empty() {
        warn(doc_id, block_line, "invalid /user block: name is empty");
        return;
    }

    if registries.users.contains_key(name) {
        warn(
            doc_id,
            block_line,
            format!("duplicate /user block for '{name}', ignoring"),
        );
        return;
    }

    let display_name = parsed
        .display_name
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    registries.users.insert(
        name.to_string(),
        User {
            name: name.to_string(),
            display_name,
        },
    );
}

#[derive(Debug, Deserialize)]
struct SubscriptionToml {
    user: String,
    endpoint: String,
    p256dh: String,
    auth: String,
}

fn parse_subscription_block(
    doc_id: &str,
    block_line: usize,
    toml_text: &str,
    registries: &mut DirectiveRegistries,
) {
    let parsed: SubscriptionToml = match toml::from_str(toml_text) {
        Ok(parsed) => parsed,
        Err(err) => {
            warn(
                doc_id,
                block_line,
                format!("invalid toml for /subscription block: {err}"),
            );
            return;
        }
    };

    let user = parsed.user.trim();
    let endpoint = parsed.endpoint.trim();
    let p256dh = parsed.p256dh.trim();
    let auth = parsed.auth.trim();

    if user.is_empty() {
        warn(
            doc_id,
            block_line,
            "invalid /subscription block: user is empty",
        );
        return;
    }

    if endpoint.is_empty() || p256dh.is_empty() || auth.is_empty() {
        warn(
            doc_id,
            block_line,
            "invalid /subscription block: endpoint, p256dh, and auth are required",
        );
        return;
    }

    registries
        .subscriptions
        .entry(user.to_string())
        .or_default()
        .push(Subscription {
            endpoint: endpoint.to_string(),
            p256dh: p256dh.to_string(),
            auth: auth.to_string(),
        });
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum NotifyTo {
    One(String),
    Many(Vec<String>),
}

#[derive(Debug, Deserialize)]
struct NotificationToml {
    to: NotifyTo,
    at: String,
    message: String,
}

fn parse_notify_block(
    doc_id: &str,
    block_line: usize,
    toml_text: &str,
    registries: &mut DirectiveRegistries,
) {
    let parsed: NotificationToml = match toml::from_str(toml_text) {
        Ok(parsed) => parsed,
        Err(err) => {
            warn(
                doc_id,
                block_line,
                format!("invalid toml for /notify block: {err}"),
            );
            return;
        }
    };

    let to = match normalize_recipients(parsed.to) {
        Ok(to) => to,
        Err(message) => {
            warn(doc_id, block_line, message);
            return;
        }
    };

    let at = match OffsetDateTime::parse(parsed.at.trim(), &Rfc3339) {
        Ok(at) => at,
        Err(err) => {
            warn(
                doc_id,
                block_line,
                format!("invalid /notify block: at must be RFC3339 ({err})"),
            );
            return;
        }
    };

    registries.notifications.push(Notification {
        to,
        at,
        message: parsed.message,
        doc_id: doc_id.to_string(),
    });
}

fn normalize_recipients(to: NotifyTo) -> Result<Vec<String>, &'static str> {
    let raw = match to {
        NotifyTo::One(value) => vec![value],
        NotifyTo::Many(values) => values,
    };
    let recipients: Vec<String> = raw
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect();

    if recipients.is_empty() {
        Err("invalid /notify block: to must include at least one user")
    } else {
        Ok(recipients)
    }
}

fn warn(doc_id: &str, line: usize, message: impl std::fmt::Display) {
    eprintln!("push directive warning: {doc_id}:{line}: {message}");
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;
    use std::pin::Pin;
    use std::sync::Mutex;
    use std::task::{Context, Poll};
    use tokio::sync::oneshot;

    #[test]
    fn load_directive_registries__should_load_directives() {
        let root = create_temp_root("load-directives");
        let contents = r#"/user
```toml
name = "marten"
display_name = "Marten"
```

/subscription
```toml
user = "marten"
endpoint = "https://push.example/123"
p256dh = "p256"
auth = "auth"
```

/notify
```toml
to = ["marten"]
at = "2025-01-12T09:30:00Z"
message = "Check the daily log."
```
"#;
        std::fs::write(root.join("note.md"), contents).expect("write note.md");

        let registries = DirectiveRegistries::load(&root).expect("load registries");

        let user = registries.users.get("marten").expect("user entry");
        assert_eq!(user.display_name.as_deref(), Some("Marten"));

        let subscriptions = registries
            .subscriptions
            .get("marten")
            .expect("subscriptions");
        assert_eq!(subscriptions.len(), 1);
        assert_eq!(subscriptions[0].endpoint, "https://push.example/123");
        assert_eq!(subscriptions[0].p256dh, "p256");
        assert_eq!(subscriptions[0].auth, "auth");

        assert_eq!(registries.notifications.len(), 1);
        let notification = &registries.notifications[0];
        assert_eq!(notification.to, vec!["marten".to_string()]);
        let expected =
            OffsetDateTime::parse("2025-01-12T09:30:00Z", &Rfc3339).expect("parse expected time");
        assert_eq!(notification.at, expected);
        assert_eq!(notification.message, "Check the daily log.");
        assert_eq!(notification.doc_id, "note.md");

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn load_directive_registries__should_ignore_invalid_blocks() {
        let root = create_temp_root("invalid-blocks");
        let contents = r#"/user
```toml
name = ""
```

/notify
```toml
to = 123
at = "not-a-time"
message = "Nope"
```
"#;
        std::fs::write(root.join("bad.md"), contents).expect("write bad.md");

        let registries = DirectiveRegistries::load(&root).expect("load registries");

        assert!(registries.users.is_empty());
        assert!(registries.notifications.is_empty());

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn load_directive_registries__should_ignore_duplicate_users() {
        let root = create_temp_root("duplicate-users");
        let contents = r#"/user
```toml
name = "marten"
display_name = "First"
```

/user
```toml
name = "marten"
display_name = "Second"
```
"#;
        std::fs::write(root.join("dup.md"), contents).expect("write dup.md");

        let registries = DirectiveRegistries::load(&root).expect("load registries");

        let user = registries.users.get("marten").expect("user entry");
        assert_eq!(user.display_name.as_deref(), Some("First"));

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn load_directive_registries__should_ignore_symlinked_markdown() {
        use std::os::unix::fs::symlink;

        let root = create_temp_root("symlink");
        let contents = r#"/user
```toml
name = "real"
```
"#;
        let target = root.join("real.md");
        std::fs::write(&target, contents).expect("write real.md");
        symlink(&target, root.join("link.md")).expect("create symlink");

        let registries = DirectiveRegistries::load(&root).expect("load registries");

        assert_eq!(registries.users.len(), 1);
        assert!(registries.users.contains_key("real"));

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    fn create_temp_root(test_name: &str) -> std::path::PathBuf {
        let mut root = std::env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        root.push(format!("mindex-{}-{}", test_name, nanos));
        std::fs::create_dir_all(&root).expect("create temp dir");
        root
    }

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
