use clap::Parser;
use std::{net::SocketAddr, path::PathBuf};

#[derive(Parser, Debug)]
#[command(
    name = "mindex",
    version,
    about = "Small markdown knowledge base server"
)]
struct Cli {
    #[arg(long)]
    root: PathBuf,
}

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

    mindex::serve(addr, root).await;
}
