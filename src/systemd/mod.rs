
use std::{future, net::{IpAddr, Ipv4Addr}};

use anyhow::{bail, Context, Result};
use futures::{stream, StreamExt, TryStreamExt};
use rtnetlink::packet_route::{address::AddressAttribute, AddressFamily};
use tracing::{error, info, warn};

async fn get_if_addr(ifname: &String) -> Result<Ipv4Addr> {
    let (connection, handle, _) = rtnetlink::new_connection()?;
    tokio::spawn(connection);

    let link = handle.link().get()
        .match_name(ifname.clone())
        .execute()
        .try_next().await?
        .context("Failed to find interface {ifname}")?;

    // Fetch link addresses
    let addrs = handle.address().get()
        .set_link_index_filter(link.header.index)
        .execute()
        // Extract attributes
        .try_filter_map(|a| future::ready(
            if a.header.family == AddressFamily::Inet {
                Ok(Some(a.attributes))
            } else {
                Ok(None)
            })
        )
        .map_ok(|attrs| stream::iter(
            attrs.into_iter()
                .map(|a| Ok::<AddressAttribute, rtnetlink::Error>(a))
        ))
        .try_flatten()
        .try_collect::<Vec<AddressAttribute>>().await?
        // Extract relevant addresses
        .into_iter()
        .flat_map(|a| if let AddressAttribute::Address(addr) = a {
            Some(addr)
        } else {
            None
        })
        .collect::<Vec<IpAddr>>();

    if addrs.len() == 0 {
        bail!("No IPv4 address found for interface {ifname}")
    }
    if addrs.len() > 1 {
        // Not possible under Linux? Check anyway.
        bail!("Multiple IPv4 addresses found on for interface {ifname}")
    }
    if let IpAddr::V4(ipaddr) = addrs[0] {
        Ok(ipaddr)
    } else {
        bail!("Found non-IPv4 address on {ifname}; this is an internal logic error")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::fs::read_to_string;
    use tracing_test::traced_test;

    #[tokio::test]
    #[traced_test]
    async fn test_fetch_addrs() -> Result<()> {
        // Hack: parse an interface address out of kernel routes
        let ifname = read_to_string("/proc/net/route").await?
            .lines()
            .skip(1) // header
            .take(1)
            .collect::<String>()
            .split_whitespace()
            .take(1)
            .collect::<String>();

        let _ip = get_if_addr(&ifname).await?;

        Ok(())
    }

}
