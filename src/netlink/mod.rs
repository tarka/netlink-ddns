
use std::{future, net::{IpAddr, Ipv4Addr}};

use anyhow::{bail, Context, Result};
use futures::{stream, TryStreamExt};
use rtnetlink::{
    packet_route::{
        address::AddressAttribute,
        AddressFamily, RouteNetlinkMessage}, proto::Connection, sys::SmolSocket, Handle};


fn new_connection() -> Result<(Connection<RouteNetlinkMessage, SmolSocket>, Handle)> {
    let (connection, handle, _) = rtnetlink::new_connection_with_socket()?;
    Ok((connection, handle))
}

pub(crate) async fn get_if_addr(ifname: &String) -> Result<Ipv4Addr> {
    let (connection, handle) = new_connection()?;

    smol::spawn(connection).detach();

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
                .map(Ok::<AddressAttribute, rtnetlink::Error>)
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

    if addrs.is_empty() {
        bail!("No IPv4 address found for interface {ifname}")
    }
    if addrs.len() > 1 {
        bail!("Multiple IPv4 addresses found on for interface {ifname}")
    }
    if let IpAddr::V4(ipaddr) = addrs[0] {
        Ok(ipaddr)
    } else {
        bail!("Found non-IPv4 address on {ifname}; this is an internal logic error")
    }
}

// pub async fn listen_for_ipv4(ifname: &String) {
//     // Open the netlink socket
//     let (mut connection, _, mut messages) =
//         new_connection().map_err(|e| format!("{e}"))?;

//     // These flags specify what kinds of broadcast messages we want to listen
//     // for.
//     let mgroup_flags = RTMGRP_LINK
//         | RTMGRP_IPV4_IFADDR
//         | RTMGRP_IPV4_ROUTE
//         | RTMGRP_IPV6_IFADDR
//         | RTMGRP_IPV6_ROUTE;

//     // A netlink socket address is created with said flags.
//     let addr = SocketAddr::new(0, mgroup_flags);
//     // Said address is bound so new conenctions and thus new message broadcasts
//     // can be received.
//     connection
//         .socket_mut()
//         .socket_mut()
//         .bind(&addr)
//         .expect("failed to bind");
//     tokio::spawn(connection);

// }


#[cfg(test)]
mod tests {
    use super::*;
    use macro_rules_attribute::apply;
    use smol::fs::read_to_string;
    use smol_macros::test;
    use tracing_test::traced_test;

    #[apply(test!)]
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
