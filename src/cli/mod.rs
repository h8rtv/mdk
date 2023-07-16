mod cmd;

use anyhow::{anyhow, Result};
use bollard::Docker;
use clap::{Parser, Subcommand};

use crate::config::MdkConfig;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    New { profile: String, name: String },
    Connect { name: String },
    Cfg,
    // TODO:
    // List containers
    // Remove/stop containers
}


pub async fn run(docker: &Docker, cfg: MdkConfig) -> Result<()> {
    let cli = Cli::parse();
    match &cli.command {
        Commands::Connect { name } => {
            cmd::connect(&docker, name).await?;
        }
        Commands::New { profile, name } => {
            match cfg.profiles.get(profile) {
                Some(profile) => Ok(cmd::create_container(&docker, name, profile).await?),
                None => Err(anyhow!("The profile does not exist")),
            }?;
        }
        Commands::Cfg => {
            println!("{}", cfg);
        }
    };
    Ok(())
}
