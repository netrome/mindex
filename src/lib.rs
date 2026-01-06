use std::net::SocketAddr;
use std::path::PathBuf;

mod app;
mod assets;
pub mod config;
mod state;
mod templates;

pub async fn serve(addr: SocketAddr, root: PathBuf, config: config::AppConfig) {
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind address");
    axum::serve(listener, app::app(root, config))
        .await
        .expect("server error");
}
