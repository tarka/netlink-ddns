use anyhow::{bail, Result};
use tracing::error;
use zone_update::async_impl::{AsyncDnsProvider, gandi::{Auth, Gandi}};

use crate::config::Config;


fn get_auth(config: &Config) -> Result<Auth> {
    // let auth = if let Some(key) = &config.gandi_api_key {
    //     Auth::ApiKey(key.clone())
    // } else if let Some(key) = &config.gandi_pat_key {
    //     Auth::PatKey(key.clone())
    // } else {
    //     error!("No Gandi key set");
    //     bail!("No Gandi key set");
    // };

    // FIXME:
    let auth = Auth::ApiKey("sjdlfja".to_string());
    Ok(auth)
}


pub fn get_dns_provider(config: &Config) -> Result<impl AsyncDnsProvider> {

    let dns_conf = zone_update::Config {
        domain: config.ddns.domain.clone(),
        dry_run: config.dry_run,
    };

    let gandi = Gandi::new(dns_conf, get_auth(&config)?);

    Ok(gandi)
}
