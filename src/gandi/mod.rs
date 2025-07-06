
mod types;

use std::{env, net::Ipv4Addr, sync::LazyLock};

use anyhow::{bail, Result};
use reqwest::{header::AUTHORIZATION, Client, StatusCode};
use serde::{de::DeserializeOwned, Serialize};
use tracing::{error, info, warn};
use types::{Error, Record, RecordUpdate};

static CLIENT: LazyLock<Client> = LazyLock::new(Client::new);

static API_KEY: LazyLock<Option<String>> = LazyLock::new(|| env::var("GANDI_APIKEY").ok());
static PAT_KEY: LazyLock<Option<String>> = LazyLock::new(|| env::var("GANDI_PATKEY").ok());

#[cfg(not(test))]
const API_BASE: &str = "https://api.gandi.net/v5";
#[cfg(test)]
const API_BASE: &str = "https://api.sandbox.gandi.net/v5";

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

async fn post<T>(url: &str, body: &T) -> Result<()>
where
    T: Serialize,
{
    info!("POST: {url}");
    let res = CLIENT.post(url)
        .header(AUTHORIZATION, get_auth()?)
        .json(body)
        .send().await?;
    if !res.status().is_success() {
        let err: Error = res.json().await?;
        error!("Gandi post failed: {}", err.message);
        bail!("Gandi post failed: {}", err.message);
    }

    Ok(())
}

pub async fn get_records(domain: &str) -> Result<Vec<Record>> {
    let url = format!("{API_BASE}/livedns/domains/{domain}/records");
    let recs = get(&url).await?
        .unwrap_or(vec![]);
    Ok(recs)
}

pub async fn get_host_ipv4(domain: &str, host: &str) -> Result<Option<Ipv4Addr>> {
    let url = format!("{API_BASE}/livedns/domains/{domain}/records/{host}/A");
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
    let url = format!("{API_BASE}/livedns/domains/{domain}/records/{host}/A");
    let update = RecordUpdate {
        rrset_values: vec![ip.to_string()],
        rrset_ttl: Some(300),
    };
    put(&url, &update).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::{types::*, *};
    use tracing_test::traced_test;

    static INIT: AtomicBool = AtomicBool::new(false);

    async fn init_domain(fqdn: &str) -> Result<()> {
        info!("INIT");
        let init = INIT.swap(true, Ordering::SeqCst);
        if init {
            return Ok(());
        }

        let url = format!("{API_BASE}/domain/domains");

        let create = CreateDomain {
            fqdn: fqdn.to_string(),
            owner: Owner {
                city: "Paris".to_string(),
                given: "Alice".to_string(),
                family: "Doe".to_string(),
                zip: "75001".to_string(),
                country: "FR".to_string(),
                streetaddr: "5 rue neuve".to_string(),
                phone: "+33.123456789".to_string(),
                state: "FR-IDF".to_string(),
                owner_type: "individual".to_string(),
                email: "alice@example.org".to_string(),
            }

        };
        post(&url, &create).await?;

        info!("INIT DONE");
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn test_fetch_records() -> Result<()> {
        println!("INIT");
        init_domain("haltcondition.net").await?;
        println!("INIT DONE");
        let recs = get_records("haltcondition.net").await?;
        assert!(recs.len() > 0);
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn test_fetch_records_error() -> Result<()> {
        init_domain("haltcondition.net").await?;
        let recs = get_records("not.a.real.domain.net").await?;
        assert!(recs.is_empty());
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn test_fetch_ipv4() -> Result<()> {
        init_domain("haltcondition.net").await?;
        let ip = get_host_ipv4("haltcondition.net", "janus").await?;
        assert!(ip.is_some());
        assert_eq!(Ipv4Addr::new(192,168,42,1), ip.unwrap());
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn test_update_ipv4() -> Result<()> {
        init_domain("haltcondition.net").await?;

        let cur = get_host_ipv4("haltcondition.net", "test").await?
            .unwrap_or(Ipv4Addr::new(1,1,1,1));
        let next = cur.octets()[0]
            .wrapping_add(1);

        let nip = Ipv4Addr::new(next,next,next,next);
        set_host_ipv4("haltcondition.net", "test", &nip).await?;

        // let ip = get_host_ipv4("haltcondition.net", "test").await?;
        // if let Some(ip) = ip {
        //     assert_eq!(nip, ip);
        // } else {
        //     assert!(false, "No updated IP found");
        // }

        Ok(())
    }

}
