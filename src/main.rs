use clap::Parser;
use std::io::Write;

mod config;
mod cli;
mod media;

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

            let output_path = output.unwrap_or(config.output.data_path.clone());
            let output_path = std::path::PathBuf::from(output_path);
            if let Some(parent) = output_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            std::fs::create_dir_all(&config.output.media_path)?;

            let f = std::fs::File::create(output_path)?;
            let mut writer = std::io::BufWriter::new(f);

            let mut first = true;
            write!(writer, "[")?;            
            for sheet in &config.spreadsheet.sheets {
                run(
                    &client,
                    &config,
                    &sheet,
                    &mut writer,
                    &mut first,
                ).await?;
            }
            write!(writer, "]")?;
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
    writer: &mut impl std::io::Write,
    first: &mut bool 
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

        // Identify the record using the identifier field
        let mut identifier: Option<String> = None;
        for field in &config.data_structure.fields {
            if field.is_identifier.unwrap_or(false) {
                identifier = record.get(&field.csv).cloned();
                break;
            }
        }
        let id = identifier
            .ok_or_else(|| anyhow::anyhow!("Missing identifier field"))?;

        let mut image_index = 1; // To handle multiple media fields(images) in the same record

        for field in &config.data_structure.fields {

            // Handle media fields
            if let Some(media) = &field.media {
                if let Some(url) = record.get(&field.csv) {
                    let path = media::make_media_path(
                        &config.output.media_path,
                        &sheet.name,
                        &id,
                        media,
                        image_index,
                        url
                    );
                    image_index += 1;

                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }

                    match media {
                        config::MediaType::Image => {
                            media::download_image(client, url, &path).await?;
                        },
                        _ => {}
                    }

                    obj.insert(
                        field.json.clone(),
                        serde_json::Value::String(
                            path.to_string_lossy().to_string()
                        ),
                    );
                }
                continue;
            }

            // Handle regular fields
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

        // Add sheet name to the object
        if let Some(sheet_field) = config.data_structure.sheet_field.clone() {
            obj.insert(sheet_field, serde_json::Value::String(sheet.name.clone()));
        }

        // Write the object to the output JSON file
        if !*first { write!(writer, ",")? }
        serde_json::to_writer(&mut *writer, &obj)?;
        *first = false;
    }

    Ok(())
}
