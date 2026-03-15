mod cli;

use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    let config = match cli::run() {
        cli::RunOutcome::Serve(config) => config,
        cli::RunOutcome::Exit(code) => {
            std::process::exit(code);
        }
    };
    let addr = SocketAddr::from(([127, 0, 0, 1], config.port));

    println!("listening on http://{addr}");
    mindex::serve(addr, config).await;
}
