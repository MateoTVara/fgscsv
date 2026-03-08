// src/config.rs
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct SheetConfig {
    pub name: String,
    pub gid: String,
}

#[derive(Serialize, Deserialize)]
pub struct SpreadsheetConfig {
    pub spreadsheet_id: String,
    pub sheets: Vec<SheetConfig>,
}

#[derive(Serialize, Deserialize, Clone)]
pub enum FieldType {
    String,
    Float,
    Int,
    Bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct FieldConfig {
    pub json: String,
    pub csv: String,
    pub r#type: FieldType,
    pub required: bool,
}

#[derive(Serialize, Deserialize)]
pub struct DataStructureConfig {
    pub sheet_field: String,
    pub fields: Vec<FieldConfig>
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub output_path: String,
    pub spreadsheet: SpreadsheetConfig,
    pub data_structure: DataStructureConfig,
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