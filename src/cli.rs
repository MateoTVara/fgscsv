// src/cli.rs
use crate::config;

#[derive(clap::ValueEnum, Clone)]
pub enum ConfigKey {
    Output,
    SpreadsheetId
}

#[derive(clap::Subcommand)]
pub enum Commands {
    /// Initialize configuration file
    Init,
    /// Set a configuration value
    Set {
        /// The configuration key to set
        key: ConfigKey,
        /// The value to set for the configuration key
        value: String,
    },
    /// Run the scraper
    Run {
        /// Optional output path to override config file per execution
        #[arg(long)]
        output: Option<String>,
    }
}

#[derive(clap::Parser)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands
}

pub fn init() -> anyhow::Result<()> {
    let config = config::Config {
        output_path: "tmp/products.json".into(),
        spreadsheet_id: "".into()
    };

    config::write_config(&config)?;
    println!("Created fgscsv.toml");

    Ok(())
}

pub fn set(key: ConfigKey, value: String) -> anyhow::Result<()> {
    let mut config = config::read_config()?;

    match key {
        ConfigKey::Output => config.output_path = value,
        ConfigKey::SpreadsheetId => config.spreadsheet_id = value,
    }

    config::write_config(&config)?;
    
    Ok(())
}