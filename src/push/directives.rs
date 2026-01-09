use crate::push_types::{DirectiveRegistries, Notification, Subscription, User};

use serde::Deserialize;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

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

#[derive(Debug, Clone)]
pub(crate) struct DirectiveWarning {
    pub(crate) doc_id: String,
    pub(crate) line: usize,
    pub(crate) message: String,
}

pub(super) fn parse_document(
    doc_id: &str,
    contents: &str,
    registries: &mut DirectiveRegistries,
) -> Vec<DirectiveWarning> {
    let lines: Vec<&str> = contents.lines().collect();
    let mut idx = 0usize;
    let mut pending: Option<PendingDirective> = None;
    let mut warnings = Vec::new();

    while idx < lines.len() {
        let line = lines[idx];
        let trimmed = line.trim();

        if let Some(kind) = parse_directive_line(trimmed) {
            if let Some(previous) = pending.take() {
                push_warning(
                    &mut warnings,
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
                    push_warning(
                        &mut warnings,
                        doc_id,
                        directive.line,
                        format!("unterminated toml block after {}", directive.kind.label()),
                    );
                } else if !is_toml_language(fence.language.as_deref()) {
                    push_warning(
                        &mut warnings,
                        doc_id,
                        directive.line,
                        format!("expected toml block after {}", directive.kind.label()),
                    );
                } else {
                    let toml_text = block_lines.join("\n");
                    handle_directive_block(
                        doc_id,
                        fence_line,
                        directive,
                        &toml_text,
                        registries,
                        &mut warnings,
                    );
                }
            }
            continue;
        }

        idx += 1;
    }

    if let Some(directive) = pending {
        push_warning(
            &mut warnings,
            doc_id,
            directive.line,
            format!("missing toml block after {}", directive.kind.label()),
        );
    }

    warnings
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
    warnings: &mut Vec<DirectiveWarning>,
) {
    match directive.kind {
        DirectiveKind::User => {
            parse_user_block(doc_id, block_line, toml_text, registries, warnings)
        }
        DirectiveKind::Subscription => {
            parse_subscription_block(doc_id, block_line, toml_text, registries, warnings)
        }
        DirectiveKind::Notify => {
            parse_notify_block(doc_id, block_line, toml_text, registries, warnings)
        }
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
    warnings: &mut Vec<DirectiveWarning>,
) {
    let parsed: UserToml = match toml::from_str(toml_text) {
        Ok(parsed) => parsed,
        Err(err) => {
            push_warning(
                warnings,
                doc_id,
                block_line,
                format!("invalid toml for /user block: {err}"),
            );
            return;
        }
    };

    let name = parsed.name.trim();
    if name.is_empty() {
        push_warning(
            warnings,
            doc_id,
            block_line,
            "invalid /user block: name is empty",
        );
        return;
    }

    if registries.users.contains_key(name) {
        push_warning(
            warnings,
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
    warnings: &mut Vec<DirectiveWarning>,
) {
    let parsed: SubscriptionToml = match toml::from_str(toml_text) {
        Ok(parsed) => parsed,
        Err(err) => {
            push_warning(
                warnings,
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
        push_warning(
            warnings,
            doc_id,
            block_line,
            "invalid /subscription block: user is empty",
        );
        return;
    }

    if endpoint.is_empty() || p256dh.is_empty() || auth.is_empty() {
        push_warning(
            warnings,
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
    warnings: &mut Vec<DirectiveWarning>,
) {
    let parsed: NotificationToml = match toml::from_str(toml_text) {
        Ok(parsed) => parsed,
        Err(err) => {
            push_warning(
                warnings,
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
            push_warning(warnings, doc_id, block_line, message);
            return;
        }
    };

    let at = match OffsetDateTime::parse(parsed.at.trim(), &Rfc3339) {
        Ok(at) => at,
        Err(err) => {
            push_warning(
                warnings,
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

fn push_warning(
    warnings: &mut Vec<DirectiveWarning>,
    doc_id: &str,
    line: usize,
    message: impl Into<String>,
) {
    warnings.push(DirectiveWarning {
        doc_id: doc_id.to_string(),
        line,
        message: message.into(),
    });
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

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
    fn parse_document__should_collect_warnings() {
        let contents = r#"/user
```toml
name = ""
```

/notify
"#;
        let mut registries = DirectiveRegistries::default();
        let warnings = parse_document("note.md", contents, &mut registries);

        assert!(warnings.iter().any(|warning| {
            warning.doc_id == "note.md"
                && warning.line == 2
                && warning.message == "invalid /user block: name is empty"
        }));
        assert!(warnings.iter().any(|warning| {
            warning.doc_id == "note.md"
                && warning.line == 6
                && warning.message == "missing toml block after /notify"
        }));
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
}
