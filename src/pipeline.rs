// src/pipeline.rs
use crate::{config, media, cli};

pub async fn run(
    client: &reqwest::Client,
    config: &config::Config,
    sheet: &config::SheetConfig,
    writer: &mut impl std::io::Write,
    first: &mut bool,
    state: &mut cli::State,
    seen: &mut std::collections::HashSet<String>,
) -> anyhow::Result<()> {
    let csv_content = fetch_csv(client, config, sheet).await?;
    let mut rdr = create_csv_reader(&csv_content);
    println!("{:#?}", rdr.headers()?);

    for result in rdr.deserialize::<std::collections::HashMap<String, String>>() {
        let record = result?;
        
        let obj = process_record(
            &record, config, client, sheet, state, seen
        ).await?;

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
    index: &mut i32,
    list: &mut Vec<serde_json::Value>
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

    list.push(serde_json::Value::String(path.to_string_lossy().to_string()));

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
    sheet: &config::SheetConfig,
    state: &mut cli::State,
    seen: &mut std::collections::HashSet<String>,
) -> anyhow::Result<serde_json::Map<String, serde_json::Value>> {
    // println!("Record: {:#?}", record);
    let mut obj = serde_json::Map::new();

    // Identify the record using the identifier field
    let id = extract_identifier(&record, config)?;
    seen.insert(id.clone());

    let prev = state.get(&id);
    let is_new = prev.is_none();

    if is_new {
        println!("NEW entry: {}", id);
    }

    let mut image_index = 1; // To handle multiple media fields(images) in the same record
    let mut video_index = 1; // To handle multiple media fields(videos) in the same record
    let mut other_index = 1;

    let mut images = vec![];
    let mut videos = vec![];
    let mut others = vec![];

    let mut changed = false;

    for field in &config.data_structure.fields {
        let field_changed = prev
            .map(|p| p.get(&field.csv) != record.get(&field.csv))
            .unwrap_or(true);

        if field_changed && !is_new {
            println!("Field '{}' changed for '{}'", field.csv, id);
        }

        changed |= field_changed;

        // Handle media fields
        if let Some(media) = &field.media {
            if field_changed {
                println!("Media field '{}' changed for '{}'", field.csv, id);
                process_media_field(
                    client, config, sheet, &record,
                    &field, media, &id,
                    match media {
                        config::MediaType::Image => &mut image_index,
                        config::MediaType::Video => &mut video_index,
                        config::MediaType::Other => &mut other_index
                    },
                    match media {
                        config::MediaType::Image => &mut images,
                        config::MediaType::Video => &mut videos,
                        config::MediaType::Other => &mut others,
                    },
                ).await?;
            }
        } else { // Handle regular fields
            process_regular_field(&record, field, sheet, &mut obj)?;
        }
    }

    if !is_new && !changed {
        println!("UNCHANGED entry: {}", id);
    }

    state.insert(id.clone(), record.clone());

    obj.insert("images".to_string(), serde_json::Value::Array(images));
    obj.insert("videos".to_string(), serde_json::Value::Array(videos));

    // Add sheet name to the object
    add_sheet_field(&config, &sheet, &mut obj);

    Ok(obj)
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