use std::net::SocketAddr;

pub mod config;

mod adapters;
mod directives;
mod documents;
mod ports;
mod types;

mod app;
mod assets;
mod push;
mod state;
mod templates;

pub struct VapidCredentials {
    pub private_key: String,
    pub public_key: String,
}

pub fn generate_vapid_credentials() -> Result<VapidCredentials, web_push::WebPushError> {
    let credentials = push::vapid::generate_vapid_credentials()?;
    Ok(VapidCredentials {
        private_key: credentials.private_key,
        public_key: credentials.public_key,
    })
}

pub async fn serve(addr: SocketAddr, config: config::AppConfig) {
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind address");
    axum::serve(listener, app::app(config))
        .await
        .expect("server error");
}
