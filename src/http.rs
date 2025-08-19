use std::{fmt::Debug, sync::Arc};

use anyhow::{bail, Result};
use futures_rustls::{
    pki_types::ServerName,
    rustls::{ClientConfig, RootCertStore},
    TlsConnector,
};
use http_body_util::BodyExt;
use hyper::{
    body::{Buf, Incoming},
    client::conn::http1,
    header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HOST},
    Request, Response, StatusCode,
};
use serde::{de::DeserializeOwned, Serialize};
use smol::net::TcpStream;
use smol_hyper::rt::FuturesIo;
use tracing::{debug, error, warn};

fn load_system_certs() -> RootCertStore {
    let mut root_store = RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    root_store
}

pub async fn request(host: &'static str, req: Request<String>) -> Result<Response<Incoming>> {
    let addr = format!("{host}:443");
    let stream = TcpStream::connect(addr).await?;

    let cert_store = load_system_certs();
    let tlsdomain = ServerName::try_from(host)?;
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



pub async fn get<T, E>(host: &'static str, endpoint: &String, auth: Option<String>) -> Result<Option<T>>
where
    T: DeserializeOwned,
    E: DeserializeOwned + Debug,
{
    debug!("Request https://{host}{endpoint}");
    let mut req = Request::get(endpoint)
        .header(HOST, host)
        .header(ACCEPT, "application/json");
    if let Some(auth) = auth {
        req = req.header(AUTHORIZATION, auth);
    }
    let res = request(host, req.body(String::new())?).await?;

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
            let err: E = serde_json::from_reader(body.reader())?;
            error!("Gandi lookup failed: {err:?}");
            bail!("Gandi lookup failed: {err:?}");
        }
    }
}
