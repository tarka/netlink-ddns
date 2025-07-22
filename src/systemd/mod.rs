
use anyhow::{bail, Result};
use tracing::{error, info, warn};

#[cfg(test)]
mod tests {
    use std::{any::Any, future};

    use super::*;
    use rtnetlink::{new_connection, packet_route::{address::{AddressAttribute, AddressMessage}, AddressFamily}};
    use tracing_test::traced_test;
    use futures::{stream::{self, TryStreamExt}, FutureExt, StreamExt};

    #[tokio::test]
    #[traced_test]
    async fn test_fetch_addrs() -> Result<()> {
        let (connection, handle, _) = new_connection()?;
        tokio::spawn(connection);

        let mut links = handle.link().get()
            .match_name("wlp10s0".to_string())
            .execute();

        while let Some(link) = links.try_next().await? {
            let attrs = handle.address().get()
                .set_link_index_filter(link.header.index)
                .execute()
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
                .try_collect::<Vec<AddressAttribute>>().await?;

//            println!("ATTR: {addrs:#?}");

            //            while let attr = attrs {
            for attr in attrs {
//                println!("ADDR: {:#?}", attr.header.family);
//                for a in addr.attributes {
                    println!("Attr: {attr:#?}");
//                }
            }
        }

        assert!(false);
        Ok(())
    }

}
