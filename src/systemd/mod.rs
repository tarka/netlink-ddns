
mod linux;

use anyhow::{Context, Result};
use tracing::{error, info, warn};
use zbus::{proxy::PropertyChanged, Connection};
use futures::StreamExt;
use zbus_systemd::network1::{LinkProxy, ManagerProxy};

pub async fn listen_for_interface_changes(interface_name: String) -> Result<()> {
    let conn = Connection::system().await?;

    // Get a proxy to the network manager
    let network_manager = ManagerProxy::new(&conn).await?;

    // Get the interface index and object path by name
    let (_ifindex, object_path) = network_manager.get_link_by_name(interface_name.clone()).await
        .context("Failed to get interface index")?;

    let interface_proxy = LinkProxy::builder(&conn)
        .path(object_path)?
        .build()
        .await?;
    //let mut properties_stream = interface_proxy.receive_i_pv4_address_state_changed().await;
    let mut properties_stream = interface_proxy.receive_administrative_state_changed().await;

    info!("Listening for property changes on interface {}", interface_name);
    while let Some(event) = properties_stream.next().await {
        let changed_properties = event;

        // Call the handler for property changes
        handle_interface_property_change(interface_name.clone(), changed_properties).await;
    }

    Ok(())
}

async fn handle_interface_property_change(interface_name: String, changed_properties: PropertyChanged<'_, String>) {
    info!("Property change detected on interface {}: {:?}", interface_name, changed_properties.name());
    let p = changed_properties.get().await.unwrap();
    info!("Change: {}", p);
}

#[cfg(test)]
mod tests {
    use super::*;
    use macro_rules_attribute::apply;
    use smol_macros::test;
    use tracing_test::traced_test;
    use zbus::{proxy::PropertyChanged, Connection};

    #[apply(test!)]
    #[traced_test]
    async fn test_zbus_connect() -> Result<()> {
        let conn = Connection::system().await?;

        Ok(())
    }

}
