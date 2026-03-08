use std::io::{BufRead, BufReader, Write};

// src/cli.rs
use crate::config;

#[derive(clap::ValueEnum, Clone)]
pub enum ConfigKey {
    Output,
    SpreadsheetId
}

#[derive(clap::Subcommand)]
pub enum Commands {
    /// Initialize configuration file, replace with your spreadsheet ID and sheets info before running
    Init,
    /// Run the scraper
    Run {
        /// Optional output path to override config file per execution
        #[arg(long)]
        output: Option<String>,
    },
    /// Set a configuration value
    Set {
        /// The configuration key to set
        key: ConfigKey,
        /// The value to set for the configuration key
        value: String,
    },
    /// Prints where the config file is located
    Path,
    /// Show the current configuration
    Show,
}

#[derive(clap::Parser)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands
}

pub fn init() -> anyhow::Result<()> {
    if std::path::Path::new("fgscsv.toml").exists() {
        anyhow::bail!("fgscsv.toml already exists");
    }

    let config = config::Config {
        output_path: "tmp/products.json".into(),
        spreadsheet: config::SpreadsheetConfig {
            spreadsheet_id: "your_spreadsheet_id_here".into(),
            sheets: vec![
                config::SheetConfig {
                    category: "Category1".into(),
                    gid: "sheet_gid_1".into(),
                },
                config::SheetConfig {
                    category: "Category2".into(),
                    gid: "sheet_gid_2".into(),
                },
            ],
        }.into(),
    };

    config::write_config(&config)?;
    println!("Created fgscsv.toml");

    Ok(())
}

pub fn set(key: ConfigKey, value: String) -> anyhow::Result<()> {
    let mut config = config::read_config()?;

    match key {
        ConfigKey::Output => config.output_path = value,
        ConfigKey::SpreadsheetId => config.spreadsheet.spreadsheet_id = value,
    }

    config::write_config(&config)?;
    
    Ok(())
}

pub fn path() -> anyhow::Result<()> {
    let path = std::fs::canonicalize("fgscsv.toml")?;
    println!("{}", path.display());
    Ok(())
}

pub fn show() -> anyhow::Result<()> {
    let path = std::path::PathBuf::from("fgscsv.toml");
    let f = std::fs::File::open(path)?;

    let out = std::io::stdout();
    let mut out = out.lock();

    for line in BufReader::new(f).lines() {
        let line = line?;
        writeln!(out, "{line}")?;
    }
    
    Ok(())
}