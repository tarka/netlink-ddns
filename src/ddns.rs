
use anyhow::Result;
use serde::Deserialize;
use zone_update::async_impl::{
    gandi, dnsimple, dnsmadeeasy, porkbun,
    AsyncDnsProvider,
};

use crate::config::Config;

// FIXME: This and get_dns_provider should probably be part of
// zone-update, but we need to define it here to control
// deserialisation from the config file. (?)
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "lowercase", tag = "name")]
pub enum Providers {
    Gandi(gandi::Auth),
    Dnsimple(dnsimple::Auth),
    DnsMadeEasy(dnsmadeeasy::Auth),
    PorkBun(porkbun::Auth),
}


type DnsProvider = Box<dyn AsyncDnsProvider>;

pub fn get_dns_provider(config: &Config) -> Result<DnsProvider> {

    let dns_conf = zone_update::Config {
        domain: config.ddns.domain.clone(),
        dry_run: config.dry_run,
    };

    let provider: DnsProvider = match config.ddns.provider.clone() {
        Providers::Gandi(auth) => Box::new(gandi::Gandi::new(dns_conf, auth)),
        Providers::Dnsimple(auth) => Box::new(dnsimple::Dnsimple::new(dns_conf, auth, None)),
        Providers::DnsMadeEasy(auth) => Box::new(dnsmadeeasy::DnsMadeEasy::new(dns_conf, auth)),
        Providers::PorkBun(auth) => Box::new(porkbun::Porkbun::new(dns_conf, auth)),
    };

    Ok(provider)
}
