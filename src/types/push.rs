#[derive(Debug, Clone)]
pub struct VapidConfig {
    pub private_key: String,
    #[allow(unused)] // TODO: Will be used when we implement subscription UI
    pub public_key: String,
    pub subject: String,
}
