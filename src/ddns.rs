
use zone_update::async_impl::AsyncDnsProvider;

use crate::config::Config;

pub fn get_dns_provider(config: &Config) -> Box<dyn AsyncDnsProvider> {

    let dns_conf = zone_update::Config {
        domain: config.ddns.domain.clone(),
        dry_run: config.dry_run,
    };

    config.ddns.provider.async_impl(dns_conf)
}
