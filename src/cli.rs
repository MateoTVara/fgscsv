// keep-sorted start
use crate::{config, dsl, pipeline};
use clap::{Parser, Subcommand};
use std::{
    collections::HashMap,
    fs,
    io::{self, Write},
};
// keep-sorted end

#[derive(Subcommand)]
pub enum Commands {
    /// Fetch and process spreadsheet data from the web according to the configuration.
    Run,

    /// Create an initial configuration file.
    Init,
}

#[derive(Parser)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

pub async fn run_cli() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = config::get_config()?;

    let mut handles = Vec::new();

    for (sheet_id, sheet) in &config.sheets {
        let raw_schema = config
            .schemas
            .get(&sheet.schema)
            .ok_or(format!("schema `{}` doesn't exist", sheet.schema))?;

        let schema = dsl::parse_schema(raw_schema)?;

        handles.push((
            sheet.schema.clone(),
            tokio::spawn(pipeline::process_sheet(
                config.root_path.clone(),
                sheet_id.clone(),
                schema,
            )),
        ));
    }

    let mut records_per_schema = HashMap::new();
    let mut new_state_per_schema = HashMap::new();

    for (schema_name, handle) in handles {
        let sheet = handle.await??;

        records_per_schema
            .entry(schema_name.clone())
            .or_insert_with(Vec::new)
            .extend(sheet.records);

        new_state_per_schema
            .entry(schema_name.clone())
            .or_insert_with(HashMap::new)
            .extend(sheet.state);
    }

    for (schema_name, new_state) in &new_state_per_schema {
        let current_state = pipeline::get_current_state(schema_name.clone())?;
        let states = pipeline::get_record_states(&current_state, new_state);

        for (record_id, state) in states {
            match state {
                pipeline::RecordState::Added { hash } => {
                    println!("{record_id} was added: {hash}");
                }

                pipeline::RecordState::Updated { old_hash, new_hash } => {
                    println!("{record_id} was updated");
                    println!("  old: {old_hash}");
                    println!("  new: {new_hash}");
                }

                pipeline::RecordState::Deleted { hash } => {
                    println!("{record_id} was deleted: {hash}");
                }
            }
        }
    }

    for (schema_name, records) in records_per_schema {
        pipeline::write_schema_data(&schema_name, &records).await?;
    }

    let state_path = std::path::Path::new(".fgscsv/state.json");
    fs::create_dir_all(".fgscsv")?;
    let file = fs::File::create(state_path)?;
    serde_json::to_writer_pretty(file, &new_state_per_schema)?;

    Ok(())
}

pub fn init_cli() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if fs::exists(config::CONFIG_FILE_PATH)? && !confirm_overwrite()? {
        println!("Exiting...");
        return Ok(());
    }

    create_initial_config()?;

    Ok(())
}

fn create_initial_config() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let initial_config = config::Config {
        root_path: String::from("url/to/your/csv/file"),
        schemas: HashMap::from([(
            "product".to_string(),
            HashMap::from([
                ("id".to_string(), "string @identifier".to_string()),
                ("name".to_string(), "string".to_string()),
                ("price".to_string(), "float".to_string()),
                ("base qty".to_string(), "string -> base_qty".to_string()),
                ("description".to_string(), "string?".to_string()),
                ("img1".to_string(), "string? @media(image)".to_string()),
            ]),
        )]),
        sheets: HashMap::from([
            (
                "1234".to_string(),
                config::Sheet {
                    name: "Products".to_string(),
                    schema: "product".to_string(),
                },
            ),
            (
                "4321".to_string(),
                config::Sheet {
                    name: "Categories".to_string(),
                    schema: "product".to_string(),
                },
            ),
        ]),
    };
    let serialized_initial_config = toml::to_string(&initial_config)?;
    fs::write(config::CONFIG_FILE_PATH, serialized_initial_config)?;
    Ok(())
}

fn confirm_overwrite() -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    loop {
        print!(
            "Existing {} file, do you want to overwrite the file? [y/N] ",
            config::CONFIG_FILE_PATH
        );
        io::stdout().flush()?;

        let mut answer = String::new();
        io::stdin().read_line(&mut answer)?;

        match answer.trim().to_lowercase().as_str() {
            "y" | "yes" => return Ok(true),
            "n" | "no" | "" => return Ok(false),
            _ => println!("Please enter y or n."),
        }
    }
}
