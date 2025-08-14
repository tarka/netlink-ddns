
mod config;
mod gandi;
mod netlink;

use anyhow::Result;
use futures::stream::StreamExt;
use tracing::{info, warn};
use tracing_subscriber::{filter::LevelFilter, EnvFilter};

fn init_logging() -> Result<()> {
    let env_log = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();

    tracing_log::LogTracer::init()?;
    let fmt = tracing_subscriber::fmt()
        .with_env_filter(env_log)
        .finish();
    tracing::subscriber::set_global_default(fmt)?;

    Ok(())
}


fn main() -> Result<()> {
    init_logging()?;

    smol::block_on(async {
        info!("Starting...");
        let config = config::get_config()?;

        let local = netlink::get_if_addr(&config.iface).await?;
        if let Some(lip) = local {
            let dns = gandi::get_host_ipv4(&config.domain, &config.host).await?;
            if local != dns {
                info!("DNS record out of date; updating");
                gandi::set_host_ipv4(&config.domain, &config.host, &lip).await?;
            }
        } else {
            warn!("No local address currently set");
        }

        let mut msgs = netlink::ipv4_addr_stream(&config.iface).await?;
        while let Some(message) = msgs.next().await {
            println!("Route change message - {message:?}");
        }

        Ok(())
    })
}
