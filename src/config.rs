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

use std::fs::read_to_string;

use anyhow::{Context, Result};
use once_cell::sync::OnceCell;
use pico_args::Arguments;
use serde::Deserialize;

use crate::ddns::Providers;


#[derive(Debug)]
pub struct CliOptions {
    /// Config file
    ///
    /// Override the config file location
    pub config: Option<String>,
}

impl CliOptions {
    pub fn from_args() -> Result<CliOptions> {
        let mut args = Arguments::from_env();

        Ok(CliOptions {
            config: args.opt_value_from_str(["-c", "--config"])?
        })

    }
}


// FIXME: Use OnceCell lib for now until OnceLock::get_or_try_init()
// stablises.
static CONFIG: OnceCell<Config> = OnceCell::new();

pub const DEFAULT_CONFIG_FILE: &str = "/etc/netlink-ddns/config.corn";


#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct Ddns {
    pub domain: String,
    pub host: String,
    pub provider: Providers,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub log_level: Option<String>,
    pub iface: String,
    pub ddns: Ddns,
    #[serde(default)]
    pub dry_run: bool,
}

pub fn get_config(cli_file: &Option<String>) -> Result<&'static Config> {
    CONFIG.get_or_try_init(|| {

        let confile = cli_file.clone()
            .unwrap_or(DEFAULT_CONFIG_FILE.to_owned());
        let conf_s = read_to_string(&confile)
            .with_context(|| format!("Failed to load config from {confile}"))?;

        let conf = corn::from_str::<Config>(&conf_s)?;
        Ok(conf)
    })
}


#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "lowercase")]
    pub struct ConfWrapper {
        pub ddns: Ddns,
    }

    #[test]
    fn test_tagged_provider() -> Result<()> {
        let fragment = r#"
            {
                ddns = {
                    provider = {
                      name = "porkbun"
                      key = "a_key"
                      secret = "a_secret"
                    }
                    domain = "example.com"
                    host = "test"
                }
            } "#;
        let conf = corn::from_str::<ConfWrapper>(fragment)?;
        assert_eq!(conf.ddns.host, "test".to_string());
        assert_eq!(conf.ddns.domain, "example.com".to_string());
        if let Providers::PorkBun(auth) = conf.ddns.provider {
            assert_eq!(auth.key, "a_key".to_string());
            assert_eq!(auth.secret, "a_secret".to_string());
        } else {
            panic!("Provider mismatch, should be PorkBun");
        }

        Ok(())
    }

    #[test]
    fn test_tagged_gandi() -> Result<()> {
        let fragment = r#"
            {
                ddns = {
                    provider = {
                      name = "gandi"
                      apikey = "api_key"
                    }
                    domain = "example.com"
                    host = "test"
                }
            } "#;
        let conf = corn::from_str::<ConfWrapper>(fragment)?;
        assert_eq!(conf.ddns.host, "test".to_string());
        assert_eq!(conf.ddns.domain, "example.com".to_string());
        if let Providers::Gandi(gandi::Auth::ApiKey(key)) = conf.ddns.provider {
            assert_eq!(key, "api_key".to_string());
        } else {
            panic!("Provider mismatch, should be PorkBun");
        }

        Ok(())
    }

    #[test]
    fn test_gandi_mixed() -> Result<()> {
        let fragment = r#"
            {
                ddns = {
                    provider = {
                      name = "gandi"
                      // Only one variant should be allowed
                      apikey = "api_key"
                      patkey = "pat_key"
                    }
                    domain = "example.com"
                    host = "test"
                }
            } "#;

        let conf_r = corn::from_str::<ConfWrapper>(fragment);
        assert!(matches!(conf_r, Err(corn::error::Error::DeserializationError(_))));
        Ok(())
    }

    #[test]
    fn test_example_config() -> Result<()> {
        let file = "examples/config.corn".to_owned();
        let conf = get_config(&Some(file))?;

        assert_eq!(conf.ddns.host, "test".to_string());
        assert_eq!(conf.ddns.domain, "example.com".to_string());
        if let Providers::PorkBun(auth) = &conf.ddns.provider {
            assert_eq!(auth.key, "a_key".to_string());
            assert_eq!(auth.secret, "a_secret".to_string());
        } else {
            panic!("Provider mismatch, should be PorkBun");
        }

        Ok(())
    }
}
