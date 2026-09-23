use std::net::SocketAddr;

use api_server::{
    app_and_runtime_host_from_env, init_tracing, parse_bind_addr, DEFAULT_API_SERVER_ADDR,
};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    // Both host metrics and the console process list scan all visible PIDs.
    // sysinfo otherwise retains one /proc/<pid>/stat descriptor per PID in
    // each sampler, so a busy development host can leave thousands open.
    #[cfg(target_os = "linux")]
    if !sysinfo::set_open_files_limit(128) {
        tracing::warn!("could not bound sysinfo process stat descriptor cache");
    }

    let addr: SocketAddr = parse_bind_addr(
        std::env::var("API_SERVER_ADDR").ok().as_deref(),
        DEFAULT_API_SERVER_ADDR,
    )?;

    let listener = TcpListener::bind(addr).await?;
    let (app, runtime_host) = app_and_runtime_host_from_env().await?;
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    let allocator_reclaimer = api_server::host_infrastructure::spawn_allocator_reclaimer();
    let served = axum::serve(listener, app)
        .with_graceful_shutdown(api_server::shutdown_signal())
        .await;
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    allocator_reclaimer.abort();
    runtime_host.stop().await?;
    served?;

    Ok(())
}
