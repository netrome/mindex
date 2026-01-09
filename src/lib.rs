use std::net::SocketAddr;

pub mod config;

mod adapters;
mod documents;
mod ports;
mod types;

mod app;
mod assets;
mod push;
mod state;
mod templates;

pub async fn serve(addr: SocketAddr, config: config::AppConfig) {
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind address");
    axum::serve(listener, app::app(config))
        .await
        .expect("server error");
}
