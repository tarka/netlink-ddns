
use std::env;

use anyhow::Result;
use config;
use once_cell::sync::OnceCell;
use serde::Deserialize;

// FIXME: Use OnceCell lib for now until OnceLock::get_or_try_init()
// stablises.
static CONFIG: OnceCell<Config> = OnceCell::new();

#[derive(Debug, Deserialize)]
pub struct Config {
    pub log_level: Option<String>,
    pub gandi_api_key: Option<String>,
    pub gandi_pat_key: Option<String>,
    pub domain: String,
    pub host: String,
    pub iface: String,
    pub dry_run: Option<bool>,
}

pub fn get_config() -> Result<&'static Config> {
    CONFIG.get_or_try_init(|| {
        let confile = env::var("NLDDNS_CONFIG")
            .unwrap_or("/etc/nlddns/config.toml".to_string());

        let conf = config::Config::builder()
            .add_source(config::File::with_name(&confile))
            .build()?;

        let s_conf = conf.try_deserialize()?;
        Ok(s_conf)
    })
}
