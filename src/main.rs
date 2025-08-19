
mod config;
mod gandi;
mod http;
mod netlink;

use std::str::FromStr;

use anyhow::Result;
use futures::stream::StreamExt;
use tracing::{info, warn};
use tracing_subscriber::{EnvFilter, filter::LevelFilter};

use crate::netlink::ChangeType;

fn init_logging(level: &Option<String>) -> Result<()> {
    let lf = level.clone()
        .map(|s| LevelFilter::from_str(&s).expect("Invalid log string"))
        .unwrap_or(LevelFilter::INFO);

    let env_log = EnvFilter::builder()
        .with_default_directive(lf.into())
        .from_env_lossy();

    tracing_log::LogTracer::init()?;
    let fmt = tracing_subscriber::fmt()
        .with_env_filter(env_log)
        .finish();
    tracing::subscriber::set_global_default(fmt)?;

    Ok(())
}

fn main() -> Result<()> {
    let config = config::get_config()?;
    init_logging(&config.log_level)?;

    smol::block_on(async {
        info!("Starting...");

        let dns = gandi::get_host_ipv4(&config.domain, &config.host).await?;
        let mut upstream = dns;

        let local = netlink::get_if_addr(&config.iface).await?;
        if let Some(lip) = local {
            if local != dns {
                info!("DNS record out of date; updating");
                gandi::set_host_ipv4(&config.domain, &config.host, &lip).await?;
            }
        } else {
            warn!("No local address currently set");
        }

        info!("Starting monitoring stream");
        let mut msgs = netlink::ipv4_addr_stream(&config.iface).await?;
        while let Some(message) = msgs.next().await {
            match message.ctype {
                ChangeType::Add => {
                    let ip = message.addr;
                    info!("Received new address: {ip}");
                    if upstream.is_some_and(|uip| uip == ip)
                    {
                        info!("IP {ip} matches upstream, skipping");
                        continue;
                    }

                    info!("Setting DNS record");
                    gandi::set_host_ipv4(&config.domain, &config.host, &ip).await?;
                    info!("DNS Set");
                    upstream = Some(ip);
                }
                ChangeType::Del => {
                    let ip = message.addr;
                    info!("IP {ip} was deleted from iface {}", config.iface);
                }
            }
        }

        Ok(())
    })
}
