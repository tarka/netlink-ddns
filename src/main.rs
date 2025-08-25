// netlink-ddns: A DDNS client on netlink
// Copyright (C) 2025 tarkasteve@gmail.com
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

mod config;
mod gandi;
mod http;
mod netlink;

use std::{str::FromStr, time::Duration};

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
    info!("Starting...");

    smol::block_on(async {
        info!("Waiting for {} to come up...", config.iface);

        let local = loop {
            let attempt = netlink::get_if_addr(&config.iface).await;
            if let Ok(Some(ip)) = attempt {
                info!("IP Addr valid on {}", config.iface);
                break ip;
            }
            warn!("Error getting IP: {attempt:?}; sleeping");
            smol::Timer::after(Duration::from_secs(10)).await;
        };

        let mut upstream = gandi::get_host_ipv4(&config.domain, &config.host).await?;

        if Some(local) != upstream {
            info!("DNS record out of date; updating");
            gandi::set_host_ipv4(&config.domain, &config.host, &local).await?;
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
