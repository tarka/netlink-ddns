
mod gandi;

use anyhow::Result;
use tokio;
use tracing::info;
use tracing_subscriber::{filter::LevelFilter, EnvFilter};

pub fn init_logging() -> Result<()> {
    let env_log = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();

    tracing_log::LogTracer::init()?;
    let fmt = tracing_subscriber::fmt()
        .with_env_filter(env_log)
        .finish();
    tracing::subscriber::set_global_default(fmt)?;

    Ok(())
}


#[tokio::main]
async fn main() -> Result<()> {
    init_logging()?;
    info!("Starting...");

    let recs = gandi::get_records("htpc.haltcondition.net").await?;
    println!("Records: {recs:?}");

    Ok(())
}
