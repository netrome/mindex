use std::net::SocketAddr;

mod app;
mod assets;
pub mod config;
pub mod push;
pub mod state;
mod templates;

pub async fn serve(addr: SocketAddr, config: config::AppConfig) {
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind address");
    axum::serve(listener, app::app(config))
        .await
        .expect("server error");
}
