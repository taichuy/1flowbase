mod native_distribution;

use std::net::SocketAddr;

use api_server::{
    app_and_runtime_host_from_env, init_tracing, parse_bind_addr, DEFAULT_API_SERVER_ADDR,
};
use tokio::net::TcpListener;

#[cfg(all(target_os = "linux", target_env = "gnu"))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

fn main() -> anyhow::Result<()> {
    // Configure the private distribution before starting runtime worker threads.
    let native = native_distribution::NativeDistribution::prepare()?;
    init_tracing();
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(run(native))
}

async fn run(native: Option<native_distribution::NativeDistribution>) -> anyhow::Result<()> {
    let mut shutdown = Box::pin(api_server::shutdown_signal());
    let native = if let Some(native) = native {
        let mut startup = tokio::task::spawn_blocking(move || native.start_postgres());
        Some(tokio::select! {
            biased;
            _ = &mut shutdown => { drop(startup.await??); return Ok(()); }
            native = &mut startup => native??,
        })
    } else {
        None
    };

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
    let (mut app, runtime_host) = tokio::select! {
        biased;
        _ = &mut shutdown => return Ok(()),
        app = app_and_runtime_host_from_env() => app?,
    };
    if let Some(native) = &native {
        app = native_distribution::static_web::mount(app, &native.web_root)?;
        native.announce_ready();
    }
    let served = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await;
    runtime_host.stop().await?;
    served?;

    Ok(())
}
