use clap::Parser;
use std::{net::SocketAddr, path::PathBuf};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let root = std::fs::canonicalize(&cli.root)
        .unwrap_or_else(|err| panic!("failed to resolve root directory: {err}"));
    if !root.is_dir() {
        panic!("root path is not a directory: {}", root.display());
    }
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    println!("listening on http://{addr}");

    let config = mindex::config::AppConfig {
        app_name: cli.app_name,
        icon_192: cli.icon_192,
        icon_512: cli.icon_512,
    };
    mindex::serve(addr, root, config).await;
}

#[derive(Parser, Debug)]
#[command(
    name = "mindex",
    version,
    about = "Small markdown knowledge base server"
)]
struct Cli {
    #[arg(long)]
    root: PathBuf,
    #[arg(long, default_value = "Mindex")]
    app_name: String,
    #[arg(long)]
    icon_192: Option<PathBuf>,
    #[arg(long)]
    icon_512: Option<PathBuf>,
}
