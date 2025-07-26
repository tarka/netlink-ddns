
use std::{future, net::{IpAddr, Ipv4Addr}};

use anyhow::{bail, Result};
use futures::{stream, TryStreamExt};
use rtnetlink::packet_route::{address::AddressAttribute, AddressFamily};
use tracing::{error, info, warn};

async fn get_if_addr(ifname: &String) -> Result<Ipv4Addr> {
    let (connection, handle, _) = rtnetlink::new_connection()?;
    tokio::spawn(connection);

    let mut links = handle.link().get()
        .match_name("wlp10s0".to_string())
        .execute();

    while let Some(link) = links.try_next().await? {
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
            // Probably not possible under Linux? Check anyway.
            bail!("Multiple IPv4 addresses found on for interface {ifname}")
        }
        for addr in addrs {
            println!("Attr: {addr:#?}");
        }
    }

    Ok(Ipv4Addr::new(1,1,1,1))
}

#[cfg(test)]
mod tests {
    use std::{future, net::IpAddr};

    use super::*;
    use rtnetlink::{new_connection, packet_route::{address::{AddressAttribute}, AddressFamily}};
    use tracing_test::traced_test;
    use futures::{stream::{self, TryStreamExt}};

    #[tokio::test]
    #[traced_test]
    async fn test_fetch_addrs() -> Result<()> {

        assert!(false);
        Ok(())
    }

}
