pub mod config;
pub mod cli;

use anyhow::Result;
use bollard::{Docker, API_DEFAULT_VERSION};

#[tokio::main]
async fn main() -> Result<()> {
    let cfg = config::get_or_create_config().await?;
    let addr = format!("http://{}", cfg.hostname);
    let docker = Docker::connect_with_http(&addr, 4, API_DEFAULT_VERSION)?;

    cli::run(&docker, cfg).await?;
    Ok(())
}
