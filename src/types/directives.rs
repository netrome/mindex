use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use time::OffsetDateTime;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DirectiveRegistries {
    pub users: HashMap<String, User>,
    pub subscriptions: HashMap<String, Vec<Subscription>>,
    pub notifications: Vec<Notification>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub name: String,
    pub display_name: Option<String>,
    pub password_hash: String,
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
