use std::{io::stdout, io::Read, io::Write, time::Duration};

use anyhow::{anyhow, Result};
use bollard::container::{Config, CreateContainerOptions, StartContainerOptions};
use bollard::exec::{CreateExecOptions, StartExecResults};
use bollard::service::ContainerCreateResponse;
use bollard::Docker;
use clap::{Parser, Subcommand};
use futures_util::StreamExt;
use termion::async_stdin;
use termion::raw::IntoRawMode;
use tokio::io::AsyncWriteExt;
use tokio::spawn;
use tokio::time::sleep;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    New { name: String },
    Connect { name: String },
}

async fn create_container(
    docker: &Docker,
    container_name: &str,
    image_name: &str,
) -> Result<ContainerCreateResponse> {
    let options = Some(CreateContainerOptions {
        name: container_name,
        platform: None,
    });

    let config = Config {
        image: Some(image_name),
        tty: Some(true),
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

#[tokio::main]
async fn main() -> Result<()> {
    let docker = Docker::connect_with_http_defaults()?;
    let cli = Cli::parse();
    match &cli.command {
        Commands::Connect { name } => {
            connect(&docker, name).await?;
        }
        Commands::New { name } => {
            create_container(&docker, name, "ubuntu:latest").await?;
        }
    };
    Ok(())
}
