use std::path::PathBuf;

#[derive(Clone)]
pub struct AppConfig {
    pub root: PathBuf,
    pub app_name: String,
    pub icon_192: Option<PathBuf>,
    pub icon_512: Option<PathBuf>,
    pub vapid_private_key: Option<String>,
    pub vapid_public_key: Option<String>,
    pub vapid_subject: Option<String>,
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
        }
    }
}
