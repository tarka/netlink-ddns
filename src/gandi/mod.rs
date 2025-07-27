
mod types;

use std::{env, net::Ipv4Addr, sync::{Arc, LazyLock}};

use anyhow::{bail, Result};
use futures_rustls::{pki_types::ServerName, rustls::{ClientConfig, RootCertStore}, TlsConnector};
use http_body_util::BodyExt;
use hyper::{body::{Buf, Incoming}, client::conn::http1, header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HOST}, Request, Response, StatusCode};
use serde::{de::DeserializeOwned, Serialize};
use smol::net::TcpStream;
use smol_hyper::rt::FuturesIo;
use tracing::{debug, error, warn};
use types::{Error, Record, RecordUpdate};

static API_KEY: LazyLock<Option<String>> = LazyLock::new(|| env::var("GANDI_APIKEY").ok());
static PAT_KEY: LazyLock<Option<String>> = LazyLock::new(|| env::var("GANDI_PATKEY").ok());

const API_HOST: &str = "api.gandi.net";
const API_BASE: &str = "/v5/livedns";

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

fn load_system_certs() -> RootCertStore {
    let mut root_store = RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    root_store
}

async fn request(req: Request<String>) -> Result<Response<Incoming>> {
    let addr = format!("{API_HOST}:443");
    let stream = TcpStream::connect(addr).await?;

    let cert_store = load_system_certs();
    let tlsdomain = ServerName::try_from(API_HOST)?;
    let tlsconf = ClientConfig::builder()
        .with_root_certificates(cert_store)
        .with_no_client_auth();
    let tlsconn = TlsConnector::from(Arc::new(tlsconf));
    let tlsstream = tlsconn.connect(tlsdomain, stream).await?;

    let (mut sender, conn) = http1::handshake(FuturesIo::new(tlsstream)).await?;

    smol::spawn(async move {
        if let Err(e) = conn.await {
            error!("Connection failed: {:?}", e);
        }
    }).detach();

    let res = sender.send_request(req).await?;

    Ok(res)
}

async fn get<T>(endpoint: &str) -> Result<Option<T>>
where
    T: DeserializeOwned,
{
    debug!("Request https://{API_HOST}{endpoint}");
    let req = Request::get(format!("{endpoint}"))
        .header(HOST, API_HOST)
        .header(AUTHORIZATION, get_auth()?)
        .header(ACCEPT, "application/json")
        .body(String::new())?;

    let res = request(req).await?;

    match res.status() {
        StatusCode::OK => {
            // Asynchronously aggregate the chunks of the body
            let body = res.collect().await?
                .aggregate();
            let obj: T = serde_json::from_reader(body.reader())?;

            Ok(Some(obj))
        }
        StatusCode::NOT_FOUND => {
            warn!("Gandi record doesn't exist: {}", endpoint);
            Ok(None)
        }
        _ => {
            let body = res.collect().await?
                .aggregate();
            let err: Error = serde_json::from_reader(body.reader())?;
            error!("Gandi lookup failed: {}", err.message);
            bail!("Gandi lookup failed: {}", err.message);
        }
    }
}

async fn put<T>(url: &str, obj: &T) -> Result<()>
where
    T: Serialize,
{
    let body = serde_json::to_string(obj)?;
    let req = Request::put(url)
        .header(HOST, API_HOST)
        .header(CONTENT_TYPE, "application/json")
        .header(ACCEPT, "application/json")
        .header(AUTHORIZATION, get_auth()?)
        .body(body)?;

    let res = request(req).await?;

    if !res.status().is_success() {
        let code = res.status();
        let body = res.collect().await?
            .aggregate();
        let err: Error = serde_json::from_reader(body.reader())?;
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
    use macro_rules_attribute::apply;
    use smol_macros::test;
    use tracing_test::traced_test;

    #[apply(test!)]
    #[traced_test]
    async fn test_fetch_records() -> Result<()> {
        let recs = get_records("haltcondition.net").await?;
        assert!(recs.len() > 0);
        Ok(())
    }

    #[apply(test!)]
    #[traced_test]
    async fn test_fetch_records_error() -> Result<()> {
        let recs = get_records("not.a.real.domain.net").await?;
        assert!(recs.is_empty());
        Ok(())
    }

    #[apply(test!)]
    #[traced_test]
    async fn test_fetch_ipv4() -> Result<()> {
        let ip = get_host_ipv4("haltcondition.net", "janus").await?;
        assert!(ip.is_some());
        assert_eq!(Ipv4Addr::new(192,168,42,1), ip.unwrap());
        Ok(())
    }

    #[apply(test!)]
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
