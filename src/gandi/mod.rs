
mod types;

use std::{env, net::Ipv4Addr, sync::LazyLock};

use anyhow::{bail, Result};
use reqwest::{header::AUTHORIZATION, Client, StatusCode};
use serde::{de::DeserializeOwned, Serialize};
use tracing::{error, warn};
use types::{Error, Record, RecordUpdate};

static CLIENT: LazyLock<Client> = LazyLock::new(Client::new);

static API_KEY: LazyLock<Option<String>> = LazyLock::new(|| env::var("GANDI_APIKEY").ok());
static PAT_KEY: LazyLock<Option<String>> = LazyLock::new(|| env::var("GANDI_PATKEY").ok());

const API_BASE: &str = "https://api.gandi.net/v5/livedns";

fn get_auth() -> Result<String> {
    let auth = if let Some(key) = API_KEY.as_ref() {
        format!("Apikey {key}")
    } else if let Some(key) = PAT_KEY.as_ref() {
        format!("Bearer {key}")
    } else {
        error!("No Gandi key set");
        bail!("No Gandi key set");
    };
    Ok(auth)
}

async fn get<T>(url: &str) -> Result<Option<T>>
where
    T: DeserializeOwned,
{
    let res = CLIENT.get(url)
        .header(AUTHORIZATION, get_auth()?)
        .send().await?;
    match res.status() {
        StatusCode::OK => {
            let recs = res.json().await?;
            Ok(Some(recs))
        }
        StatusCode::NOT_FOUND => {
            warn!("Gandi record doesn't exist: {}", url);
            Ok(None)
        }
        _ => {
            let err: Error = res.json().await?;
            error!("Gandi lookup failed: {}", err.message);
            bail!("Gandi lookup failed: {}", err.message);
        }
    }
}

async fn put<T>(url: &str, body: &T) -> Result<()>
where
    T: Serialize,
{
    let res = CLIENT.put(url)
        .header(AUTHORIZATION, get_auth()?)
        .json(body)
        .send().await?;
    if !res.status().is_success() {
        let code = res.status();
        let err: Error = res.json().await?;
        error!("Gandi update failed: {} {}", code, err.message);
        bail!("Gandi update failed: {} {}", code, err.message);
    }

    Ok(())
}

pub async fn get_records(domain: &str) -> Result<Vec<Record>> {
    let url = format!("{API_BASE}/domains/{domain}/records");
    let recs = get(&url).await?
        .unwrap_or(vec![]);
    Ok(recs)
}

pub async fn get_host_ipv4(domain: &str, host: &str) -> Result<Option<Ipv4Addr>> {
    let url = format!("{API_BASE}/domains/{domain}/records/{host}/A");
    let rec: Record = match get(&url).await? {
        Some(rec) => rec,
        None => return Ok(None)
    };

    let nr = rec.rrset_values.len();

    // FIXME: Assumes no or single address (which probably makes sense
    // for DDNS, but may cause issues with malformed zones.
    if nr > 1 {
        error!("Returned number of IPs is {}, should be 1", nr);
        bail!("Returned number of IPs is {}, should be 1", nr);
    } else if nr == 0 {
        warn!("No IP returned for {host}, continuing");
        return Ok(None);
    }

    let ip = rec.rrset_values[0].parse()?;
    Ok(Some(ip))
}

pub async fn set_host_ipv4(domain: &str, host: &str, ip: &Ipv4Addr) -> Result<()> {
    let url = format!("{API_BASE}/domains/{domain}/records/{host}/A");
    let update = RecordUpdate {
        rrset_values: vec![ip.to_string()],
        rrset_ttl: Some(300),
    };
    put(&url, &update).await?;
    Ok(())
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
        let recs = get_records("not.a.real.domain.net").await?;
        assert!(recs.is_empty());
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn test_fetch_ipv4() -> Result<()> {
        let ip = get_host_ipv4("haltcondition.net", "janus").await?;
        assert!(ip.is_some());
        assert_eq!(Ipv4Addr::new(192,168,42,1), ip.unwrap());
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn test_update_ipv4() -> Result<()> {
        let cur = get_host_ipv4("haltcondition.net", "test").await?
            .unwrap_or(Ipv4Addr::new(1,1,1,1));
        let next = cur.octets()[0]
            .wrapping_add(1);

        let nip = Ipv4Addr::new(next,next,next,next);
        set_host_ipv4("haltcondition.net", "test", &nip).await?;

        let ip = get_host_ipv4("haltcondition.net", "test").await?;
        if let Some(ip) = ip {
            assert_eq!(nip, ip);
        } else {
            assert!(false, "No updated IP found");
        }

        Ok(())
    }

}
