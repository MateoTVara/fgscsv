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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsl::{Field, FieldType, FieldValue, Schema};

    // Helper to build a schema from a map for testing
    fn build_schema(fields: Vec<(&str, Field)>) -> Schema {
        let mut map = HashMap::new();
        for (name, field) in fields {
            map.insert(name.to_string(), field);
        }
        Schema { fields: map }
    }

    #[test]
    fn deserialize_record_ok() {
        let schema = build_schema(vec![
            (
                "id",
                Field {
                    ty: FieldType::String,
                    nullable: false,
                    rename: None,
                    meta: None,
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

        let result = deserialize_record(record, &schema).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(
            result.get("id").unwrap(),
            &Some(FieldValue::String("abc".to_string()))
        );
        assert_eq!(result.get("cost").unwrap(), &Some(FieldValue::Float(12.5)));
    }

    #[test]
    fn deserialize_record_nullable_empty() {
        let schema = build_schema(vec![(
            "qty",
            Field {
                ty: FieldType::Int,
                nullable: true,
                rename: None,
                meta: None,
            },
        )]);

        let mut record = HashMap::new();
        record.insert("qty".to_string(), "".to_string());

        let result = deserialize_record(record, &schema).unwrap();
        assert_eq!(result.get("qty").unwrap(), &None);
    }

    #[test]
    fn deserialize_record_non_nullable_empty_error() {
        let schema = build_schema(vec![(
            "qty",
            Field {
                ty: FieldType::Int,
                nullable: false,
                rename: None,
                meta: None,
            },
        )]);

        let mut record = HashMap::new();
        record.insert("qty".to_string(), "".to_string());

        let err = deserialize_record(record, &schema).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("not nullable") && msg.contains("empty value"));
    }

    #[test]
    fn deserialize_record_missing_key_error() {
        let schema = build_schema(vec![(
            "id",
            Field {
                ty: FieldType::String,
                nullable: false,
                rename: None,
                meta: None,
            },
        )]);

        let record = HashMap::new(); // no "id" key

        let err = deserialize_record(record, &schema).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("unexpected key `id`"));
    }

    #[test]
    fn process_sheet_content_ok() {
        let schema = build_schema(vec![
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

        let csv_data = "name,age\nAlice,30\nBob,\nCharlie,25";
        let result = process_sheet_content(csv_data, schema).unwrap();

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
        let schema = build_schema(vec![(
            "age",
            Field {
                ty: FieldType::Int,
                nullable: false,
                rename: None,
                meta: None,
            },
        )]);

        let csv_data = "age\nnot_a_number";
        let result = process_sheet_content(csv_data, schema);
        assert!(result.is_err());
    }
}
