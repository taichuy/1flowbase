use std::net::SocketAddr;

use runtime_extension_host::{app, init_tracing, parse_bind_addr, DEFAULT_PLUGIN_RUNNER_ADDR};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    init_tracing();

    let addr: SocketAddr = parse_bind_addr(
        std::env::var("PLUGIN_RUNNER_ADDR").ok().as_deref(),
        DEFAULT_PLUGIN_RUNNER_ADDR,
    );

    let listener = TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app()).await.unwrap();
}
