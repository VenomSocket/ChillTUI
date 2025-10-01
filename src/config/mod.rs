use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    pub chill_api_key: Option<String>,
    pub putio_oauth_token: Option<String>,
    pub putio_folder_id: Option<u64>,
    pub putio_folder_name: String,
}

impl Config {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let config_path = Self::config_path()?;

        if !config_path.exists() {
            return Ok(Config {
                putio_folder_name: "ChillTUI".to_string(),
                ..Default::default()
            });
        }

        let content = fs::read_to_string(&config_path)?;
        let config: Config = serde_json::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config_path = Self::config_path()?;

        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(self)?;
        fs::write(&config_path, json)?;
        Ok(())
    }

    pub fn config_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
        let dirs = directories::ProjectDirs::from("", "", "chilltui")
            .ok_or("Could not determine config directory")?;
        Ok(dirs.config_dir().join("config.json"))
    }

    pub fn needs_setup(&self) -> bool {
        self.chill_api_key.is_none() || self.putio_oauth_token.is_none()
    }
}