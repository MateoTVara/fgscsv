// src/cli.rs
use std::io::{BufRead, BufReader, Write};
use crate::{config, pipeline};

pub type State = std::collections::HashMap<String, std::collections::HashMap<String, String>>;

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
        output: config::OutputConfig {
            data_path: "tmp/output/data.json".into(),
            media_path: "tmp/output/media".into(),
        },
        spreadsheet: config::SpreadsheetConfig {
            spreadsheet_id: "your_spreadsheet_id_here".into(),
            sheets: vec![
                config::SheetConfig {
                    name: "Category1".into(),
                    gid: "sheet_gid_1".into(),
                },
                config::SheetConfig {
                    name: "Category2".into(),
                    gid: "sheet_gid_2".into(),
                },
            ],
        },
        data_structure: config::DataStructureConfig {
            sheet_field: Some(String::from("category")),
            fields: vec![
                config::FieldConfig {
                    is_identifier: Some(true),
                    json: "id".into(),
                    csv: "id".into(),
                    r#type: config::FieldType::String,
                    required: true,
                    media: None,
                },
                config::FieldConfig {
                    is_identifier: None,
                    json: "name".into(),
                    csv: "nombre".into(),
                    r#type: config::FieldType::String,
                    required: true,
                    media: None,
                },
            ],
        },
    };

    config::write_config(&config)?;
    println!("Created fgscsv.toml");

    Ok(())
}

pub async fn run(
    output: Option<String>
) -> anyhow::Result<()> {
    let config = config::read_config()?;
    let client = reqwest::Client::new();

    let output_path = output.unwrap_or(config.output.data_path.clone());
    let output_path = std::path::PathBuf::from(output_path);

    std::fs::create_dir_all(&config.output.media_path)?;

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;

        let f = std::fs::File::create(&output_path)?;
        let mut writer = std::io::BufWriter::new(f);

        let state_path = parent.join(".fgscsv_state.json");
        let state_f = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&state_path)?;
        let state_rd = BufReader::new(state_f);
        let state: State = serde_json::from_reader(state_rd).unwrap_or_default();
        let mut state = state;

        println!("Resuming from state: {:#?}", state);

        let mut first = true;

        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        write!(writer, "[")?;
        for sheet in &config.spreadsheet.sheets {
            pipeline::run(
                &client,
                &config,
                &sheet,
                &mut writer,
                &mut first,
                &mut state,
                &mut seen,
            ).await?;
        }
        write!(writer, "]")?;

        for id in state.keys().cloned().collect::<Vec<_>>() {
            if !seen.contains(&id) {
                println!("Record '{}' deleted", id);
                state.remove(&id);
            }
        }

        let state_file = std::fs::File::create(&state_path)?;
        serde_json::to_writer_pretty(state_file, &state)?;
    }

    Ok(())
}

pub fn set(key: ConfigKey, value: String) -> anyhow::Result<()> {
    let mut config = config::read_config()?;

    match key {
        ConfigKey::Output => config.output.data_path = value,
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