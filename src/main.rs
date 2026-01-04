use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    println!("listening on http://{addr}");

    mindex::serve(addr).await;
}
