// keep-sorted start
use crate::dsl;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs};
// keep-sorted end

pub const CONFIG_FILE_PATH: &str = "fgscsv.toml";

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub root_path: String,
    pub schemas: HashMap<String, dsl::RawSchema>,
    pub sheets: HashMap<String, Sheet>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Sheet {
    pub name: String,
    pub schema: String,
}

pub fn get_config() -> Result<Config, Box<dyn std::error::Error + Send + Sync>> {
    let file_content = fs::read_to_string(CONFIG_FILE_PATH)?;
    let config: Config = toml::from_str(&file_content)?;
    Ok(config)
}
