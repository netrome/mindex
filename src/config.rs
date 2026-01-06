use std::path::PathBuf;

#[derive(Clone)]
pub struct AppConfig {
    pub root: PathBuf,
    pub app_name: String,
    pub icon_192: Option<PathBuf>,
    pub icon_512: Option<PathBuf>,
}

#[cfg(test)]
impl Default for AppConfig {
    fn default() -> Self {
        Self {
            root: "/".into(),
            app_name: "Mindex".to_string(),
            icon_192: None,
            icon_512: None,
        }
    }
}
