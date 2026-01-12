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
use std::{
    future,
    net::{IpAddr, Ipv4Addr},
};

use anyhow::{bail, Context, Result};
use futures::{
    channel::mpsc::{unbounded, UnboundedReceiver},
    stream, SinkExt, StreamExt, TryStreamExt,
};
use netlink_sys::{AsyncSocket, SocketAddr};
use rtnetlink::{
    constants::RTMGRP_IPV4_IFADDR,
    new_connection_with_socket,
    packet_core::NetlinkPayload,
    packet_route::{
        address::{AddressAttribute, AddressMessage},
        AddressFamily, RouteNetlinkMessage,
    },
    sys::SmolSocket,
};
use tracing::{debug, warn};

/// Represents the type of IP address change.
#[derive(Debug)]
pub enum ChangeType {
    /// An IP address was added to the interface
    Add,
    /// An IP address was removed from the interface
    Del,
}

/// Represents a change in IP address on a network interface.
#[derive(Debug)]
pub struct IpAddrChange {
    /// The type of change (addition or deletion)
    pub ctype: ChangeType,
    /// The name of the network interface where the change occurred
    #[allow(dead_code)]
    pub iface: String,
    /// The IPv4 address that was added or removed
    pub addr: Ipv4Addr,
}

/// Retrieves the IPv4 address of a network interface.
///
/// This function queries the system for the IPv4 address assigned to the specified
/// network interface. It returns `None` if no IPv4 address is found, or an error
/// if multiple IPv4 addresses are found or if the interface doesn't exist.
///
/// # Arguments
///
/// * `ifname` - The name of the network interface to query (e.g., "eth0", "wlan0")
///
/// # Returns
///
/// Returns a `Result` containing an `Option<Ipv4Addr>`:
/// * `Ok(Some(addr))` - Successfully retrieved the IPv4 address
/// * `Ok(None)` - No IPv4 address found for the interface
/// * `Err(...)` - An error occurred (interface not found, multiple addresses, etc.)
///
/// # Errors
///
/// This function will return an error if:
/// * The specified interface doesn't exist
/// * Multiple IPv4 addresses are found on the interface
/// * Other system-level errors occur during the query
pub(crate) async fn get_if_addr(ifname: &str) -> Result<Option<Ipv4Addr>> {
    let (connection, handle, _msgs) =
        new_connection_with_socket::<SmolSocket>()?;

    compio::runtime::spawn(connection)
        .detach();

    let link = handle
        .link()
        .get()
        .match_name(ifname.to_string())
        .execute()
        .try_next().await?
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
        .try_collect::<Vec<AddressAttribute>>().await?
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
        warn!("No IPv4 address found for interface {ifname}");
        Ok(None)
    } else if addrs.len() > 1 {
        bail!("Multiple IPv4 addresses found on for interface {ifname}")
    } else if let IpAddr::V4(ipaddr) = addrs[0] {
        Ok(Some(ipaddr))
    } else {
        bail!("Found non-IPv4 address on {ifname}; this is an internal logic error")
    }
}

/// Creates a stream that monitors IPv4 address changes on a specific network interface.
///
/// This function sets up a netlink socket to listen for IPv4 address additions and deletions
/// on the specified interface. It returns an unbounded receiver that will receive
/// `IpAddrChange` notifications when addresses are added or removed.
///
/// # Arguments
///
/// * `ifname` - The name of the network interface to monitor (e.g., "eth0", "wlan0")
///
/// # Returns
///
/// Returns a `Result` containing an `UnboundedReceiver<IpAddrChange>` that will receive
/// notifications about IP address changes, or an error if the netlink connection fails.
///
/// # Example
///
/// ```rust
/// use netlink_ddns::netlink::ipv4_addr_stream;
///
/// # async fn example() -> anyhow::Result<()> {
/// let stream = ipv4_addr_stream("eth0").await?;
/// # Ok(())
/// # }
/// ```
pub async fn ipv4_addr_stream(ifname: &'static str) -> Result<UnboundedReceiver<IpAddrChange>> {
    let addr = SocketAddr::new(0, RTMGRP_IPV4_IFADDR);

    let (mut connection, _handle, mut nlmsgs) =
        new_connection_with_socket::<SmolSocket>()?;
    let (mut tx, rx) = unbounded();

    connection
        .socket_mut()
        .socket_mut()
        .bind(&addr)?;

    compio::runtime::spawn(connection)
        .detach();

    compio::runtime::spawn(async move {
        while let Some((message, _)) = nlmsgs.next().await {
            match message.payload {
                NetlinkPayload::InnerMessage(msg) => {
                    debug!("Got payload: {msg:?}");
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

fn is_our_if(ifname: &str, addr: &AddressMessage) -> bool {
    addr.attributes.iter()
        .find_map(|attr| {
            match attr {
                AddressAttribute::Label(l) => Some(l),
                _ => None,
            }
        })
        .is_some_and(|nif| nif == ifname)
}

fn get_ip(amsg: &AddressMessage) -> Option<Ipv4Addr> {
    let v4s = amsg.attributes.iter()
        .filter_map(|attr| {
            match attr {
                AddressAttribute::Address(IpAddr::V4(ip)) => Some(*ip),
                _ => None,
            }
        })
        .collect::<Vec<Ipv4Addr>>();

    match v4s.len() {
        0 => None,
        1 => Some(v4s[0]),
        _ => {
            warn!("More that 1 IPv4 address found; not updating: {v4s:?}");
            None
        }
    }
}

fn filter_msg(ifname: &str, msg: RouteNetlinkMessage) -> Option<IpAddrChange> {
    match msg {
        RouteNetlinkMessage::NewAddress(ref amsg)
            if is_our_if(ifname, amsg) =>
        {
            get_ip(amsg)
                .map(|addr| IpAddrChange {
                    ctype: ChangeType::Add,
                    iface: ifname.to_owned(),
                    addr,
                })
        }
        RouteNetlinkMessage::DelAddress(ref amsg)
            if is_our_if(ifname, amsg) =>
        {
            get_ip(amsg)
                .map(|addr| IpAddrChange {
                    ctype: ChangeType::Del,
                    iface: ifname.to_owned(),
                    addr,
                })
        }
        _ => {
            warn!("Unexpected RouteNetlinkMessage: {msg:?}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_fs::read_to_string;
    use tracing_test::traced_test;
    use rtnetlink::packet_route::address::{AddressAttribute, AddressMessage};
    use std::net::{IpAddr, Ipv4Addr};

    #[compio::test]
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
        assert_eq!(result, None);
    }
}
