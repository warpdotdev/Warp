use anyhow::Result;
use tracing_subscriber::EnvFilter;
use warp_shim_server::{config::ShimConfig, server};

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let config = ShimConfig::from_sources()?;
    server::serve(config).await
}

fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("warp_shim_server=info"));

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .init();
}
