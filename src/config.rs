use std::path::PathBuf;

pub struct AppConfig {
    pub app_name: String,
    pub icon_192: Option<PathBuf>,
    pub icon_512: Option<PathBuf>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            app_name: "Mindex".to_string(),
            icon_192: None,
            icon_512: None,
        }
    }
}
