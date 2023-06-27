use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::{io::stdout, io::Read, io::Write, time::Duration};

use anyhow::{anyhow, Result};
use bollard::container::{Config, CreateContainerOptions, StartContainerOptions};
use bollard::exec::{CreateExecOptions, StartExecResults};
use bollard::image::CreateImageOptions;
use bollard::service::{ContainerCreateResponse, CreateImageInfo, HostConfig};
use bollard::{Docker, API_DEFAULT_VERSION};
use clap::{Parser, Subcommand};
use futures_util::{StreamExt, TryStreamExt};
use termion::async_stdin;
use termion::raw::IntoRawMode;
use tokio::io::AsyncWriteExt;
use tokio::time::sleep;
use tokio::{fs, spawn};

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
}
#[derive(Serialize, Deserialize)]
struct Profile {
    image: String,
    gpu: bool,
    // ...
}

#[derive(Serialize, Deserialize)]
struct MdkConfig {
    profiles: HashMap<String, Profile>,
    hostname: String,
}

impl std::default::Default for MdkConfig {
    fn default() -> Self {
        let mut profiles = HashMap::new();
        profiles.insert(
            "ubuntu".into(),
            Profile {
                image: "ubuntu:latest".into(),
                gpu: false,
            },
        );
        Self {
            profiles,
            hostname: "localhost:2375".into(),
        }
    }
}

async fn create_container(
    docker: &Docker,
    container_name: &str,
    profile: &Profile,
) -> Result<ContainerCreateResponse> {
    let options = Some(CreateContainerOptions {
        name: container_name,
        platform: None,
    });

    let _: Vec<CreateImageInfo> = docker
        .create_image(
            Some(CreateImageOptions {
                from_image: profile.image.to_owned(),
                ..Default::default()
            }),
            None,
            None,
        )
        .try_collect()
        .await?;

    let config = Config {
        image: Some(profile.image.to_owned()),
        tty: Some(true),
        host_config: Some(HostConfig {
            runtime: Some("nvidia".into()),
            ..Default::default()
        }),
        ..Default::default()
    };

    let response = docker.create_container(options, config).await?;
    docker
        .start_container(container_name, None::<StartContainerOptions<String>>)
        .await?;
    Ok(response)
}

async fn connect(docker: &Docker, container_name: &str) -> Result<()> {
    let config = CreateExecOptions {
        cmd: Some(vec!["bash"]),
        attach_stdout: Some(true),
        attach_stdin: Some(true),
        attach_stderr: Some(true),
        tty: Some(true),
        ..Default::default()
    };
    let message = docker.create_exec(container_name, config).await?;

    let response = docker.start_exec(&message.id, None).await?;
    match response {
        StartExecResults::Attached {
            mut output,
            mut input,
        } => {
            spawn(async move {
                let mut stdin = async_stdin().bytes();
                loop {
                    if let Some(Ok(byte)) = stdin.next() {
                        input.write(&[byte]).await.ok();
                    } else {
                        sleep(Duration::from_nanos(10)).await;
                    }
                }
            });

            // set stdout in raw mode so we can do tty stuff
            let stdout = stdout();
            let mut stdout = stdout.lock().into_raw_mode()?;

            // pipe docker exec output into stdout
            while let Some(Ok(output)) = output.next().await {
                stdout.write_all(output.into_bytes().as_ref())?;
                stdout.flush()?;
            }
            Ok(())
        }
        _ => Err(anyhow!("Couldn't attach to the container")),
    }
}

async fn get_or_create_config() -> Result<MdkConfig> {
    match home::home_dir() {
        Some(homepath) => {
            let filepath = homepath.join(".mdk");
            if filepath.exists() {
                let file = fs::read_to_string(filepath).await?;
                Ok(serde_yaml::from_str(&file)?)
            } else {
                let cfg = MdkConfig {
                    ..Default::default()
                };
                let cfg_str = serde_yaml::to_string(&cfg)?;
                fs::write(filepath, cfg_str).await?;
                Ok(cfg)
            }
        }
        None => Err(anyhow!("Couldn't find the home directory")),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cfg = get_or_create_config().await?;
    let addr = format!("http://{}", cfg.hostname);
    let docker = Docker::connect_with_http(&addr, 4, API_DEFAULT_VERSION)?;
    let cli = Cli::parse();
    match &cli.command {
        Commands::Connect { name } => {
            connect(&docker, name).await?;
        }
        Commands::New { profile, name } => {
            match cfg.profiles.get(profile) {
                Some(profile) => Ok(create_container(&docker, name, profile).await?),
                None => Err(anyhow!("The profile does not exist")),
            }?;
        }
        Commands::Cfg => {
            println!("{}", serde_yaml::to_string(&cfg)?);
        }
    };
    Ok(())
}
