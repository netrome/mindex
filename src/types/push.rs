#[derive(Debug, Clone)]
pub struct VapidConfig {
    pub private_key: String,
    pub public_key: String,
    pub subject: String,
}
