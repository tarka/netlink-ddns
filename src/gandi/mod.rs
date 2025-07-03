
mod types;

use std::{env, sync::LazyLock};

use anyhow::{bail, Result};
use reqwest::{header::AUTHORIZATION, Client};
use types::{Error, Record};

static CLIENT: LazyLock<Client> = LazyLock::new(Client::new);

static API_KEY: LazyLock<Option<String>> = LazyLock::new(|| env::var("GANDI_APIKEY").ok());
static PAT_KEY: LazyLock<Option<String>> = LazyLock::new(|| env::var("GANDI_PATKEY").ok());

const API_BASE: &'static str = "https://api.gandi.net/v5/livedns";

pub async fn get_records(fqdn: &str) -> Result<Vec<Record>> {
    let url = format!("{API_BASE}/domains/{fqdn}/records");
    let auth = if let Some(key) = API_KEY.as_ref() {
        format!("Apikey {key}")
    } else if let Some(key) = PAT_KEY.as_ref() {
        format!("Bearer {key}")
    } else {
        bail!("No Gandi key set");
    };
    let res = CLIENT.get(url)
        .header(AUTHORIZATION, auth)
        .send().await?;
    if !res.status().is_success() {
        let err: Error = res.json().await?;
        bail!("Gandi lookup failed: {}", err.message);
    }
    let recs = res.json().await?;

    Ok(recs)
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

}
