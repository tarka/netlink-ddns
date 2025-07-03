
mod types;

use std::{env, net::Ipv4Addr, sync::LazyLock};

use anyhow::{bail, Result};
use reqwest::{header::AUTHORIZATION, Client};
use serde::de::DeserializeOwned;
use tracing::error;
use types::{Error, Record};

static CLIENT: LazyLock<Client> = LazyLock::new(Client::new);

static API_KEY: LazyLock<Option<String>> = LazyLock::new(|| env::var("GANDI_APIKEY").ok());
static PAT_KEY: LazyLock<Option<String>> = LazyLock::new(|| env::var("GANDI_PATKEY").ok());

const API_BASE: &'static str = "https://api.gandi.net/v5/livedns";

async fn get<T>(url: &str) -> Result<T>
where T: DeserializeOwned
{
    let auth = if let Some(key) = API_KEY.as_ref() {
        format!("Apikey {key}")
    } else if let Some(key) = PAT_KEY.as_ref() {
        format!("Bearer {key}")
    } else {
        error!("No Gandi key set");
        bail!("No Gandi key set");
    };
    let res = CLIENT.get(url)
        .header(AUTHORIZATION, auth)
        .send().await?;
    if !res.status().is_success() {
        let err: Error = res.json().await?;
        error!("Gandi lookup failed: {}", err.message);
        bail!("Gandi lookup failed: {}", err.message);
    }
    let recs = res.json().await?;

    Ok(recs)
}

pub async fn get_records(domain: &str) -> Result<Vec<Record>> {
    let url = format!("{API_BASE}/domains/{domain}/records");
    let recs = get(&url).await?;
    Ok(recs)
}

pub async fn get_host_ipv4(domain: &str, host: &str) -> Result<Ipv4Addr> {
    //  https://api.gandi.net/v5/livedns/domains/{fqdn}/records/{rrset_name}/{rrset_type}
    let url = format!("{API_BASE}/domains/{domain}/records/{host}/A");
    let rec: Record = get(&url).await?;

    // FIXME: Assumes single address (which probably makes sense for
    // DDNS, but may cause issues with malformed zones.
    if rec.rrset_values.len() != 1 {
        let n = rec.rrset_values.len();
        error!("Returned number of IPs is {}, should be 1", n);
        bail!("Returned number of IPs is {}, should be 1", n);
    }
    let ip = rec.rrset_values[0].parse()?;

    Ok(ip)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_test::traced_test;

    #[tokio::test]
    #[traced_test]
    async fn test_fetch_records() -> Result<()> {
        let recs = get_records("haltcondition.net").await?;
        assert!(recs.len() > 0);
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn test_fetch_records_error() -> Result<()> {
        let result = get_records("not.a.real.domain.net").await;
        assert!(result.is_err());
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn test_fetch_ipv4() -> Result<()> {
        let ip = get_host_ipv4("haltcondition.net", "janus").await?;
        assert_eq!(Ipv4Addr::new(192,168,42,1), ip);
        Ok(())
    }

}
