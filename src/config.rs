use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Display};
use tokio::fs;

#[derive(Serialize, Deserialize)]
pub struct MdkConfig {
    pub profiles: HashMap<String, Profile>,
    pub hostname: String,
}

#[derive(Serialize, Deserialize)]
pub struct Profile {
    pub image: String,
    pub gpu: bool,
    // ...
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
impl Display for MdkConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let config_str = serde_yaml::to_string(&self).map_err(|_| std::fmt::Error)?;
        f.write_str(&config_str)?;
        Ok(())
    }
}

pub async fn get_or_create_config() -> Result<MdkConfig> {
    let filepath = match home::home_dir() {
        Some(homepath) => Ok(homepath.join(".mdk")),
        None => Err(anyhow!("Couldn't find the home directory")),
    }?;

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
