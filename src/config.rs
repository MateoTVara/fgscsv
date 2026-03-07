// src/config.rs
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub output_path: String,
    pub spreadsheet_id: String,
}

const CONFIG_FILE: &str = "fgscsv.toml";

pub fn read_config() -> anyhow::Result<Config> {
    let content = std::fs::read_to_string(CONFIG_FILE)?;
    let config = toml::from_str(&content)?;
    Ok(config)
}

pub fn write_config(config: &Config) -> anyhow::Result<()> {
    let content = toml::to_string_pretty(config)?;
    std::fs::write(CONFIG_FILE, content)?;
    Ok(())
}