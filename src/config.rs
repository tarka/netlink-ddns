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

use std::env;

use anyhow::Result;
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
            .unwrap_or("/etc/netlink-ddns/config.toml".to_string());

        let conf = config::Config::builder()
            .add_source(config::File::with_name(&confile))
            .build()?;

        let s_conf = conf.try_deserialize()?;
        Ok(s_conf)
    })
}