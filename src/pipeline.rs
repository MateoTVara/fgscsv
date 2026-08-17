// keep-sorted start
use crate::dsl;
use std::collections::HashMap;
// keep-sorted end

pub async fn process_sheet(
    root_path: String,
    sheet_id: String,
    schema: dsl::Schema,
) -> Result<Vec<HashMap<String, Option<dsl::FieldValue>>>, Box<dyn std::error::Error + Send + Sync>>
{
    let sheet_content = fetch_sheet(&root_path, &sheet_id).await?;
    process_sheet_content(&sheet_content, schema)
}

async fn fetch_sheet(
    root_path: &str,
    sheet_id: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    Ok(reqwest::get(format!("{root_path}{sheet_id}"))
        .await?
        .text()
        .await?)
}

fn process_sheet_content(
    sheet_content: &str,
    schema: dsl::Schema,
) -> Result<Vec<HashMap<String, Option<dsl::FieldValue>>>, Box<dyn std::error::Error + Send + Sync>>
{
    let mut rdr = csv::Reader::from_reader(sheet_content.as_bytes());
    let mut records: Vec<HashMap<String, Option<dsl::FieldValue>>> = Vec::new();

    for result in rdr.deserialize::<HashMap<String, String>>() {
        let record = result?;
        let record = deserialize_record(record, &schema)?;
        records.push(record);
    }
    // let serialized = serde_json::to_string_pretty(&records)?;
    // println!("{}", serialized);
    // Ok(serialized)
    Ok(records)
}

pub async fn write_schema_data(
    schema_name: &str,
    records: &[HashMap<String, Option<dsl::FieldValue>>],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let serialized = serde_json::to_string_pretty(records)?;
    tokio::fs::write(format!("data_{schema_name}.json"), serialized).await?;
    Ok(())
}

pub fn deserialize_record(
    record: HashMap<String, String>,
    schema: &dsl::Schema,
) -> Result<HashMap<String, Option<dsl::FieldValue>>, Box<dyn std::error::Error + Send + Sync>> {
    let mut output = HashMap::new();
    for (csv_name, field) in &schema.fields {
        let raw_value = record
            .get(csv_name)
            .ok_or(format!("unexpected key `{csv_name}` in record"))?;

        let value = if raw_value.is_empty() {
            if field.nullable {
                None
            } else {
                return Err(
                    format!("field `{csv_name}` is not nullable but has an empty value").into(),
                );
            }
        } else {
            Some(field.ty.parse(raw_value)?)
        };

        let output_key = field.rename.as_deref().unwrap_or(csv_name);

        output.insert(output_key.to_owned(), value);
    }

    Ok(output)
}
