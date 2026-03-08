use clap::Parser;
// use serde::{Deserialize, Serialize};

mod config;
mod cli;

// #[derive(Debug, Serialize, Deserialize)]
// struct Product {
//     id: String,
//     nombre: String,
//     categoria: Option<String>,
//     precio: f32,
//     descripcion: String,
//     img1: Option<String>,
//     img2: Option<String>
// }

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = cli::Cli::parse();

    match args.command {
        cli::Commands::Init => {
            cli::init()?;
        },
        cli::Commands::Run { output } => {
            let config = config::read_config()?;

            let client = reqwest::Client::new();
            let mut products: Vec<serde_json::Value> = vec![];

            for sheet in &config.spreadsheet.sheets {
                run(
                    &client,
                    &config,
                    &sheet,
                    &mut products
                ).await?;
            }

            let output_path = output.unwrap_or(config.output_path);
            let output_path = std::path::PathBuf::from(output_path);

            if let Some(parent) = output_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let f = std::fs::File::create(output_path)?;
            serde_json::to_writer_pretty(f, &products)?;
        },
        cli::Commands::Set { key, value } => {
            cli::set(key, value)?;
        },
        cli::Commands::Path => {
            cli::path()?;
        },
        cli::Commands::Show => {
            cli::show()?;
        }
    }

    Ok(())
}

async fn run(
    client: &reqwest::Client,
    config: &config::Config,
    sheet: &config::SheetConfig,
    buffer: &mut Vec<serde_json::Value>
) -> anyhow::Result<()> {
    let res = client.get(format!(
        "https://docs.google.com/spreadsheets/d/{}/export?format=csv&gid={}",
        config.spreadsheet.spreadsheet_id, sheet.gid
    )).send().await?;
    
    let csv_content = res.text().await?;

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(b',')
        .from_reader(csv_content.as_bytes());

    println!("{:#?}", rdr.headers()?);

    for result in rdr.deserialize::<std::collections::HashMap<String, String>>() {
        let record = result?;
        println!("Record: {:#?}", record);

        let mut obj = serde_json::Map::new();

        for field in &config.data_structure.fields {
            match record.get(&field.csv) {
                Some(value) => {
                    let json_value = match field.r#type {
                        config::FieldType::String =>
                            serde_json::Value::String(value.clone()),

                        config::FieldType::Float =>
                            serde_json::Value::from(value.parse::<f64>()?),

                        config::FieldType::Int =>
                            serde_json::Value::from(value.parse::<i64>()?),

                        config::FieldType::Bool =>
                            serde_json::Value::from(value.parse::<bool>()?),
                    };
                    obj.insert(field.json.clone(), json_value);
                },
                None => {
                    if field.required {
                        anyhow::bail!("Missing required field '{}' in sheet '{}'", field.csv, sheet.name);
                    }
                }
            }
        }

        obj.insert(
            config.data_structure.sheet_field.clone(),
            serde_json::Value::String(sheet.name.clone())
        );

        buffer.push(serde_json::Value::Object(obj));
    }

    Ok(())
}
