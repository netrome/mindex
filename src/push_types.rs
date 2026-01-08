use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use time::OffsetDateTime;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DirectiveRegistries {
    pub users: HashMap<String, User>,
    pub subscriptions: HashMap<String, Vec<Subscription>>,
    pub notifications: Vec<Notification>,
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
