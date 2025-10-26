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

use std::path::PathBuf;

use anyhow::Result;
use once_cell::sync::OnceCell;
use pico_args::Arguments;
use serde::Deserialize;


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

pub const DEFAULT_CONFIG_FILE: &str = "/etc/netlink-dns/config.toml";

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

pub fn get_config(cli_file: &Option<String>) -> Result<&'static Config> {
    CONFIG.get_or_try_init(|| {

        let confile = cli_file.clone()
            .unwrap_or(DEFAULT_CONFIG_FILE.to_owned());

        let conf = config::Config::builder()
            .add_source(config::File::with_name(&confile))
            .build()?;

        let s_conf = conf.try_deserialize()?;
        Ok(s_conf)
    })
}
