use std::path::PathBuf;

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) root: PathBuf,
    pub(crate) app_name: String,
    pub(crate) manifest: String,
    pub(crate) icon_192: Vec<u8>,
    pub(crate) icon_512: Vec<u8>,
}
