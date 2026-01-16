use std::path::PathBuf;
use time::Duration;

#[derive(Clone)]
pub struct AppConfig {
    pub root: PathBuf,
    pub app_name: String,
    pub icon_192: Option<PathBuf>,
    pub icon_512: Option<PathBuf>,
    pub vapid_private_key: Option<String>,
    pub vapid_public_key: Option<String>,
    pub vapid_subject: Option<String>,
    pub auth: Option<AuthConfig>,
    pub git_allowed_remote_roots: Vec<PathBuf>,
}

#[derive(Clone)]
pub struct AuthConfig {
    pub key: String,
    pub token_ttl: Duration,
    pub cookie_name: String,
    pub cookie_secure: bool,
}

#[cfg(test)]
impl Default for AppConfig {
    fn default() -> Self {
        Self {
            root: "/".into(),
            app_name: "Mindex".to_string(),
            icon_192: None,
            icon_512: None,
            vapid_private_key: None,
            vapid_public_key: None,
            vapid_subject: None,
            auth: None,
            git_allowed_remote_roots: Vec::new(),
        }
    }
}
