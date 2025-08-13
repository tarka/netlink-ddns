use std::{
    future,
    net::{IpAddr, Ipv4Addr},
};

use anyhow::{Context, Result, bail};
use futures::{channel::mpsc::{unbounded, UnboundedReceiver}, stream, SinkExt, StreamExt, TryStreamExt};
use netlink_sys::{AsyncSocket, SocketAddr};
use rtnetlink::{
    constants::RTMGRP_IPV4_IFADDR, new_connection_with_socket,
    packet_core::NetlinkPayload, packet_route::{
        address::{AddressAttribute, AddressMessage}, AddressFamily, RouteNetlinkMessage
    },
    sys::SmolSocket
};
use tracing::{info, warn};

#[derive(Debug)]
pub struct IpAddrChange {
    iface: String,
    addr: Ipv4Addr,
}

pub(crate) async fn get_if_addr(ifname: &str) -> Result<Ipv4Addr> {
    let (connection, handle, _msgs) =
        new_connection_with_socket::<SmolSocket>()?;

    smol::spawn(connection)
        .detach();

    let link = handle
        .link()
        .get()
        .match_name(ifname.to_string())
        .execute()
        .try_next()
        .await?
        .context("Failed to find interface {ifname}")?;

    // Fetch link addresses
    let addrs = handle
        .address()
        .get()
        .set_link_index_filter(link.header.index)
        .execute()
        // Extract attributes
        .try_filter_map(|a| {
            future::ready(if a.header.family == AddressFamily::Inet {
                Ok(Some(a.attributes))
            } else {
                Ok(None)
            })
        })
        .map_ok(|attrs| {
            stream::iter(
                attrs
                    .into_iter()
                    .map(Ok::<AddressAttribute, rtnetlink::Error>),
            )
        })
        .try_flatten()
        .try_collect::<Vec<AddressAttribute>>()
        .await?
        // Extract relevant addresses
        .into_iter()
        .flat_map(|a| {
            if let AddressAttribute::Address(addr) = a {
                Some(addr)
            } else {
                None
            }
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

pub async fn ipv4_addr_stream(ifname: &'static str) -> Result<UnboundedReceiver<IpAddrChange>> {
    let addr = SocketAddr::new(0, RTMGRP_IPV4_IFADDR);

    let (mut connection, _handle, mut nlmsgs) =
        new_connection_with_socket::<SmolSocket>()?;
    let (mut tx, rx) = unbounded();

    connection
        .socket_mut()
        .socket_mut()
        .bind(&addr)?;

    smol::spawn(connection)
        .detach();

     smol::spawn(async move {
        while let Some((message, _)) = nlmsgs.next().await {
            match message.payload {
                NetlinkPayload::InnerMessage(msg) => {
                    info!("Got payload: {msg:#?}");
                    if let Some(m) = filter_msg(ifname, msg) {
                        tx.send(m).await.unwrap();
                    }
                }
                _ => {
                    // According to https://docs.kernel.org/userspace-api/netlink/intro.html:
                    //
                    //   This is a unidirectional form of communication (kernel -> user)
                    //   and does not involve any control messages like NLMSG_ERROR or NLMSG_DONE.
                    //
                    warn!("Unexpected netlink message: {message:?}");
                }
            }
        }
    })
    .detach();

    Ok(rx)
}


// NewAddress(
//     AddressMessage {
//         header: AddressHeader {
//             family: Inet,
//             prefix_len: 32,
//             flags: AddressHeaderFlags(
//                 Permanent,
//             ),
//             scope: Universe,
//             index: 2,
//         },
//         attributes: [
//             Address(
//                 10.1.1.1,
//             ),
//             Local(
//                 10.1.1.1,
//             ),
//             Label(
//                 "test0",
//             ),
//             Flags(
//                 AddressFlags(
//                     Permanent,
//                 ),
//             ),
//             CacheInfo(
//                 CacheInfo {
//                     ifa_preferred: 4294967295,
//                     ifa_valid: 4294967295,
//                     cstamp: 138208,
//                     tstamp: 138208,
//                 },
//             ),
//         ],
//     },
// )

fn is_our_if(ifname: &str, addr: &AddressMessage) -> bool {
    addr.attributes.iter()
        .find_map(|attr| {
            match attr {
                AddressAttribute::Label(l) => Some(l),
                _ => None,
            }
        })
        .map_or(false, |nif| nif == ifname)
}

fn get_ip(amsg: &AddressMessage) -> Option<Ipv4Addr> {
    amsg.attributes.iter()
        .find_map(|attr| {
            match attr {
                AddressAttribute::Address(IpAddr::V4(ip)) => Some(ip.clone()),
                _ => None,
            }
        })
}

fn filter_msg(ifname: &str, msg: RouteNetlinkMessage) -> Option<IpAddrChange> {
    info!("Received Message: {msg:?}");
    match msg {
        RouteNetlinkMessage::NewAddress(ref amsg)
            if is_our_if(ifname, amsg) =>
        {
            get_ip(amsg)
                .map(|addr| IpAddrChange {
                    iface: ifname.to_owned(),
                    addr,
                })
        }
        RouteNetlinkMessage::DelAddress(ref amsg)
            if is_our_if(ifname, amsg) =>
        {
            warn!("Received Deleted Address message, but not actioning: {msg:#?}");
            None
        }
        _ => {
            warn!("Unexpected RouteNetlinkMessage: {msg:#?}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use macro_rules_attribute::apply;
    use smol::fs::read_to_string;
    use smol_macros::test;
    use tracing_test::traced_test;
    use rtnetlink::packet_route::address::{AddressAttribute, AddressMessage};
    use std::net::{IpAddr, Ipv4Addr};

    #[apply(test!)]
    #[traced_test]
    async fn test_fetch_addrs() -> Result<()> {
        // Hack: parse an interface address out of kernel routes
        let ifname = read_to_string("/proc/net/route")
            .await?
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

    #[test]
    fn test_is_our_if_matches() {
        let ifname = "eth0";

        let mut addr = AddressMessage::default();
        addr.attributes = vec![
            AddressAttribute::Label("eth0".to_string()),
        ];

        assert!(is_our_if(ifname, &addr));
    }

    #[test]
    fn test_is_our_if_no_match() {
        let ifname = "eth0";

        let mut addr = AddressMessage::default();
        addr.attributes = vec![
            AddressAttribute::Label("wlan0".to_string()),
        ];

        assert!(!is_our_if(ifname, &addr));
    }

    #[test]
    fn test_is_our_if_no_label() {
        let ifname = "eth0";

        let mut addr = AddressMessage::default();
        addr.attributes = vec![
            AddressAttribute::Address(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))),
        ];

        assert!(!is_our_if(ifname, &addr));
    }

    #[test]
    fn test_is_our_if_empty_attributes() {
        let ifname = "eth0";

        let mut addr = AddressMessage::default();
        addr.attributes = vec![];

        assert!(!is_our_if(ifname, &addr));
    }

    #[test]
    fn test_is_our_if_multiple_attributes() {
        let ifname = "eth0";

        let mut addr = AddressMessage::default();
        addr.attributes = vec![
            AddressAttribute::Address(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))),
            AddressAttribute::Label("eth0".to_string()),
            AddressAttribute::Local(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))),
        ];

        assert!(is_our_if(ifname, &addr));
    }

    #[test]
    fn test_get_ip_with_ipv4_address() {
        let mut addr = AddressMessage::default();
        let expected_ip = Ipv4Addr::new(192, 168, 1, 1);
        addr.attributes = vec![
            AddressAttribute::Label("eth0".to_string()),
            AddressAttribute::Address(IpAddr::V4(expected_ip)),
            AddressAttribute::Local(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))),
        ];

        let result = get_ip(&addr);
        assert_eq!(result, Some(expected_ip));
    }

    #[test]
    fn test_get_ip_with_no_address_attribute() {
        let mut addr = AddressMessage::default();
        addr.attributes = vec![
            AddressAttribute::Label("eth0".to_string()),
            AddressAttribute::Local(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))),
        ];

        let result = get_ip(&addr);
        assert_eq!(result, None);
    }

    #[test]
    fn test_get_ip_with_ipv6_address() {
        let mut addr = AddressMessage::default();
        addr.attributes = vec![
            AddressAttribute::Label("eth0".to_string()),
            AddressAttribute::Address(IpAddr::V6(std::net::Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1))),
        ];

        let result = get_ip(&addr);
        assert_eq!(result, None);
    }

    #[test]
    fn test_get_ip_with_empty_attributes() {
        let mut addr = AddressMessage::default();
        addr.attributes = vec![];

        let result = get_ip(&addr);
        assert_eq!(result, None);
    }

    #[test]
    fn test_get_ip_multiple_ipv4_addresses() {
        let mut addr = AddressMessage::default();
        let expected_ip = Ipv4Addr::new(192, 168, 1, 1);
        addr.attributes = vec![
            AddressAttribute::Address(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))),
            AddressAttribute::Address(IpAddr::V4(expected_ip)),
            AddressAttribute::Local(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))),
        ];

        // Should return the first IPv4 address found
        let result = get_ip(&addr);
        assert_eq!(result, Some(Ipv4Addr::new(10, 0, 0, 1)));
    }
}
