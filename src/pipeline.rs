// keep-sorted start
use crate::dsl;
use std::{
    collections::{BTreeMap, HashMap},
    fs, io,
};
// keep-sorted end

pub type Record = BTreeMap<String, Option<dsl::FieldValue>>;
pub enum RecordState {
    Added { hash: String },
    Updated { old_hash: String, new_hash: String },
    Deleted { hash: String },
}
// HashMap<record_id, state_hash>
pub type SheetState = HashMap<String, String>;
pub struct SheetResult {
    pub records: Vec<Record>,
    pub state: SheetState,
}

pub async fn process_sheet(
    root_path: String,
    sheet_id: String,
    schema: dsl::Schema,
) -> Result<SheetResult, Box<dyn std::error::Error + Send + Sync>> {
    let sheet_content = fetch_sheet(&root_path, &sheet_id).await?;
    let processed_content = process_sheet_content(&sheet_content, &schema)?;

    let new_state = get_new_state(&processed_content, &schema.identifier)?;

    // {
    //     let current_state = get_current_state(schema_name)?;
    //     let states = get_record_states(&current_state, &new_state);
    //     for (record_id, state) in states {
    //         match state {
    //             RecordState::Added { hash } => {
    //                 println!("{record_id} was added: {hash}");
    //             }

    //             RecordState::Updated { old_hash, new_hash } => {
    //                 println!("{record_id} was updated");
    //                 println!("  old: {old_hash}");
    //                 println!("  new: {new_hash}");
    //             }

    //             RecordState::Deleted { hash } => {
    //                 println!("{record_id} was deleted: {hash}");
    //             }
    //         }
    //     }
    // }

    Ok(SheetResult {
        records: processed_content,
        state: new_state,
    })
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
    schema: &dsl::Schema,
) -> Result<Vec<Record>, Box<dyn std::error::Error + Send + Sync>> {
    let mut rdr = csv::Reader::from_reader(sheet_content.as_bytes());
    let mut records: Vec<Record> = Vec::new();

    for result in rdr.deserialize::<HashMap<String, String>>() {
        let raw_record = result?;
        let record = process_record(raw_record, &schema)?;
        records.push(record);
    }

    Ok(records)
}

pub async fn write_schema_data(
    schema_name: &str,
    records: &[Record],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let serialized = serde_json::to_string_pretty(records)?;
    tokio::fs::write(format!("data_{schema_name}.json"), serialized).await?;
    Ok(())
}

pub fn process_record(
    record: HashMap<String, String>,
    schema: &dsl::Schema,
) -> Result<Record, Box<dyn std::error::Error + Send + Sync>> {
    let mut output = BTreeMap::new();
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

pub fn get_record_states(
    old_state: &SheetState,
    new_state: &SheetState,
) -> HashMap<String, RecordState> {
    let mut states = HashMap::new();

    for (record_id, new_hash) in new_state {
        match old_state.get(record_id) {
            None => {
                states.insert(
                    record_id.into(),
                    RecordState::Added {
                        hash: new_hash.clone(),
                    },
                );
            }
            Some(old_hash) if old_hash != new_hash => {
                states.insert(
                    record_id.into(),
                    RecordState::Updated {
                        old_hash: old_hash.clone(),
                        new_hash: new_hash.clone(),
                    },
                );
            }
            Some(_) => {}
        }
    }

    for (record_id, old_hash) in old_state {
        if !new_state.contains_key(record_id) {
            states.insert(
                record_id.clone(),
                RecordState::Deleted {
                    hash: old_hash.clone(),
                },
            );
        }
    }

    states
}

pub fn get_current_state(
    schema_name: String,
) -> Result<SheetState, Box<dyn std::error::Error + Send + Sync>> {
    let state_path = std::path::Path::new(".fgscsv/state.json");

    let state: HashMap<String, SheetState> = match fs::File::open(state_path) {
        Ok(file) => serde_json::from_reader(file)?,
        Err(err) if err.kind() == io::ErrorKind::NotFound => HashMap::new(),
        Err(err) => return Err(err.into()),
    };

    Ok(state.get(&schema_name).cloned().unwrap_or_default())
}

pub fn get_new_state(
    records: &[Record],
    schema_id: &str,
) -> Result<SheetState, Box<dyn std::error::Error + Send + Sync>> {
    let mut output: SheetState = HashMap::new();

    for record in records {
        let state_hash = get_record_state_hash(record)?;

        let record_id_value = record
            .get(schema_id)
            .ok_or("identifier field is missing from record")?
            .as_ref()
            .ok_or("identifier field has no value")?;

        let record_id = match record_id_value {
            dsl::FieldValue::String(value) => value.clone(),
            dsl::FieldValue::Int(value) => value.to_string(),
            dsl::FieldValue::Float(value) => value.to_string(),
            dsl::FieldValue::Bool(value) => value.to_string(),
        };

        output.insert(record_id, state_hash);
    }

    Ok(output)
}

pub fn get_record_state_hash(
    record_content: &Record,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let serialized = serde_json::to_vec(record_content)?;
    let hash = blake3::hash(&serialized);
    Ok(hash.to_hex().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsl::{Field, FieldMeta, FieldType, FieldValue, Schema};

    // Helper to build a schema from a map for testing
    fn build_schema(fields: Vec<(&str, Field)>) -> Schema {
        let identifier = fields
            .iter()
            .find(|(_name, field)| field.meta == Some(FieldMeta::Identifier))
            .map(|(name, _)| (*name).to_string())
            .unwrap();

        let mut map = HashMap::new();

        for (name, field) in fields {
            map.insert(name.to_string(), field);
        }

        Schema {
            identifier,
            fields: map,
        }
    }

    #[test]
    fn process_record_ok() {
        let schema = build_schema(vec![
            (
                "id",
                Field {
                    ty: FieldType::String,
                    nullable: false,
                    rename: None,
                    meta: Some(FieldMeta::Identifier),
                },
            ),
            (
                "price",
                Field {
                    ty: FieldType::Float,
                    nullable: true,
                    rename: Some("cost".to_string()),
                    meta: None,
                },
            ),
        ]);

        let mut record = HashMap::new();
        record.insert("id".to_string(), "abc".to_string());
        record.insert("price".to_string(), "12.5".to_string());

        let result = process_record(record, &schema).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(
            result.get("id").unwrap(),
            &Some(FieldValue::String("abc".to_string()))
        );
        assert_eq!(result.get("cost").unwrap(), &Some(FieldValue::Float(12.5)));
    }

    #[test]
    fn process_record_nullable_empty() {
        let schema = build_schema(vec![
            (
                "id",
                Field {
                    ty: FieldType::String,
                    nullable: false,
                    rename: None,
                    meta: Some(FieldMeta::Identifier),
                },
            ),
            (
                "qty",
                Field {
                    ty: FieldType::Int,
                    nullable: true,
                    rename: None,
                    meta: None,
                },
            ),
        ]);

        let mut record = HashMap::new();
        record.insert("id".to_string(), "1".to_string());
        record.insert("qty".to_string(), "".to_string());

        let result = process_record(record, &schema).unwrap();
        assert_eq!(result.get("qty").unwrap(), &None);
    }

    #[test]
    fn process_record_non_nullable_empty_error() {
        let schema = build_schema(vec![
            (
                "id",
                Field {
                    ty: FieldType::String,
                    nullable: false,
                    rename: None,
                    meta: Some(FieldMeta::Identifier),
                },
            ),
            (
                "qty",
                Field {
                    ty: FieldType::Int,
                    nullable: false,
                    rename: None,
                    meta: None,
                },
            ),
        ]);

        let mut record = HashMap::new();
        record.insert("id".to_string(), "123".to_string());
        record.insert("qty".to_string(), "".to_string());

        let err = process_record(record, &schema).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("not nullable") && msg.contains("empty value"));
    }

    #[test]
    fn process_record_missing_key_error() {
        let schema = build_schema(vec![(
            "id",
            Field {
                ty: FieldType::String,
                nullable: false,
                rename: None,
                meta: Some(FieldMeta::Identifier),
            },
        )]);

        let record = HashMap::new(); // no "id" key

        let err = process_record(record, &schema).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("unexpected key `id`"));
    }

    #[test]
    fn process_sheet_content_ok() {
        let schema = build_schema(vec![
            (
                "id",
                Field {
                    ty: FieldType::String,
                    nullable: false,
                    rename: None,
                    meta: Some(FieldMeta::Identifier),
                },
            ),
            (
                "name",
                Field {
                    ty: FieldType::String,
                    nullable: false,
                    rename: None,
                    meta: None,
                },
            ),
            (
                "age",
                Field {
                    ty: FieldType::Int,
                    nullable: true,
                    rename: None,
                    meta: None,
                },
            ),
        ]);

        let csv_data = "id,name,age\n1,Alice,30\n2,Bob,\n3,Charlie,25";
        let result = process_sheet_content(csv_data, &schema).unwrap();

        assert_eq!(result.len(), 3);
        // First record
        let r0 = &result[0];
        assert_eq!(
            r0.get("name").unwrap(),
            &Some(FieldValue::String("Alice".to_string()))
        );
        assert_eq!(r0.get("age").unwrap(), &Some(FieldValue::Int(30)));
        // Second record with empty age -> None
        let r1 = &result[1];
        assert_eq!(
            r1.get("name").unwrap(),
            &Some(FieldValue::String("Bob".to_string()))
        );
        assert_eq!(r1.get("age").unwrap(), &None);
        // Third
        let r2 = &result[2];
        assert_eq!(
            r2.get("name").unwrap(),
            &Some(FieldValue::String("Charlie".to_string()))
        );
        assert_eq!(r2.get("age").unwrap(), &Some(FieldValue::Int(25)));
    }

    #[test]
    fn process_sheet_content_error_on_invalid_type() {
        let schema = build_schema(vec![
            (
                "id",
                Field {
                    ty: FieldType::String,
                    nullable: false,
                    rename: None,
                    meta: Some(FieldMeta::Identifier),
                },
            ),
            (
                "age",
                Field {
                    ty: FieldType::Int,
                    nullable: false,
                    rename: None,
                    meta: None,
                },
            ),
        ]);

        let csv_data = "age\nnot_a_number";
        let result = process_sheet_content(csv_data, &schema);
        assert!(result.is_err());
    }
}
