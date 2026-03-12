// src/pipeline.rs
use crate::{config, media};

pub async fn run(
    client: &reqwest::Client,
    config: &config::Config,
    sheet: &config::SheetConfig,
    writer: &mut impl std::io::Write,
    first: &mut bool 
) -> anyhow::Result<()> {
    let csv_content = fetch_csv(client, config, sheet).await?;
    let mut rdr = create_csv_reader(&csv_content);

    println!("{:#?}", rdr.headers()?);

    for result in rdr.deserialize::<std::collections::HashMap<String, String>>() {
        let record = result?;
        
        let obj = process_record(&record, config, client, sheet).await?;

        // Write the object to the output JSON file
        write_json_object(first, writer, &obj)?;
    }

    Ok(())
}


async fn fetch_csv(
    client: &reqwest::Client,
    config: &config::Config,
    sheet: &config::SheetConfig,
) -> anyhow::Result<String> {
    let url = format!(
        "https://docs.google.com/spreadsheets/d/{}/export?format=csv&gid={}",
        config.spreadsheet.spreadsheet_id, sheet.gid
    );

    let res = client.get(url).send().await?;
    Ok(res.text().await?)
}

fn create_csv_reader(csv_content: &str) -> csv::Reader<&[u8]> {
    csv::ReaderBuilder::new()
        .delimiter(b',')
        .from_reader(csv_content.as_bytes())
}

fn extract_identifier(
    record: &std::collections::HashMap<String, String>,
    config: &config::Config
) -> anyhow::Result<String> {
    for field in &config.data_structure.fields {
        if field.is_identifier.unwrap_or(false) {
            if let Some(id) = record.get(&field.csv) {
                return Ok(id.clone());
            }
        }
    }
    anyhow::bail!("Missing identifier field")
}

fn add_sheet_field(
    config: &config::Config,
    sheet: &config::SheetConfig,
    obj: &mut serde_json::Map<String, serde_json::Value>,
) {
    if let Some(sheet_field) = &config.data_structure.sheet_field {
        obj.insert(sheet_field.clone(), serde_json::Value::String(sheet.name.clone()));
    }
}

fn write_json_object (
    first: &mut bool,
    writer: &mut impl std::io::Write,
    obj: &serde_json::Map<String, serde_json::Value>,
) -> anyhow::Result<()> {
    if !*first { write!(writer, ",")? }
    serde_json::to_writer(&mut *writer, &obj)?;
    *first = false;
    
    Ok(())
}

async fn process_media_field(
    client: &reqwest::Client,
    config: &config::Config,
    sheet: &config::SheetConfig,
    record: &std::collections::HashMap<String, String>,
    field: &config::FieldConfig,
    media: &config::MediaType,
    id: &str,
    obj: &mut serde_json::Map<String, serde_json::Value>,
    index: &mut i32,
) -> anyhow::Result<()> {
    let Some(url) = record.get(&field.csv) else { return Ok(()); };
    if url.is_empty() { return Ok(()); }

    let path = media::make_media_path(
        &config.output.media_path,
        &sheet.name, id, media, *index, url,
    );

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    match media {
        config::MediaType::Image => media::download_image(client, url, &path).await?,
        config::MediaType::Video => media::download_video(url, &path)?,
        _ => {}
    }

    *index += 1;

    obj.insert(field.json.clone(), serde_json::Value::String(path.to_string_lossy().to_string()));

    Ok(())
}

fn process_regular_field(
    record: &std::collections::HashMap<String, String>,
    field: &config::FieldConfig,
    sheet: &config::SheetConfig,
    obj: &mut serde_json::Map<String, serde_json::Value>,
) -> anyhow::Result<()> {

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
        }

        None => {
            if field.required {
                anyhow::bail!(
                    "Missing required field '{}' in sheet '{}'",
                    field.csv, sheet.name
                );
            }
        }
    }

    Ok(())
}

async fn process_record (
    record: &std::collections::HashMap<String, String>,
    config: &config::Config,
    client: &reqwest::Client,
    sheet: &config::SheetConfig
) -> anyhow::Result<serde_json::Map<String, serde_json::Value>> {
    println!("Record: {:#?}", record);
    let mut obj = serde_json::Map::new();

    // Identify the record using the identifier field
    let id = extract_identifier(&record, config)?;

    let mut image_index = 1; // To handle multiple media fields(images) in the same record
    let mut video_index = 1; // To handle multiple media fields(videos) in the same record
    let mut other_index = 1;

    for field in &config.data_structure.fields {
        // Handle media fields
        if let Some(media) = &field.media {
            process_media_field(
                client, config, sheet, &record,
                &field, media, &id, &mut obj,
                match media {
                    config::MediaType::Image => &mut image_index,
                    config::MediaType::Video => &mut video_index,
                    config::MediaType::Other => &mut other_index
                },
            ).await?;
        } else { // Handle regular fields
            process_regular_field(&record, field, sheet, &mut obj)?;
        }
    }

    // Add sheet name to the object
    add_sheet_field(&config, &sheet, &mut obj);

    Ok(obj)
}