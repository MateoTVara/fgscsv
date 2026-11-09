// keep-sorted start
use crate::{
    dsl::{self, Record},
    state, Result,
};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
// keep-sorted end

pub struct SheetResult {
    pub records: Vec<Record>,
    pub state: state::SheetState,
}

pub async fn process_sheet(
    root_path: String,
    sheet_id: String,
    schema: dsl::Schema,
) -> Result<SheetResult> {
    let sheet_content = fetch_sheet(&root_path, &sheet_id).await?;
    let processed_content = process_sheet_content(&sheet_content, &schema)?;
    let new_state = state::get_new_state(&processed_content, &schema.identifier)?;

    Ok(SheetResult {
        records: processed_content,
        state: new_state,
    })
}

async fn fetch_sheet(root_path: &str, sheet_id: &str) -> Result<String> {
    Ok(reqwest::get(format!("{root_path}{sheet_id}"))
        .await?
        .text()
        .await?)
}

fn process_sheet_content(sheet_content: &str, schema: &dsl::Schema) -> Result<Vec<Record>> {
    let mut rdr = csv::Reader::from_reader(sheet_content.as_bytes());
    let mut records = Vec::new();

    for result in rdr.deserialize::<HashMap<String, String>>() {
        let raw_record = result?;
        let record = process_record(raw_record, schema)?;
        records.push(record);
    }

    Ok(records)
}

pub fn data_file_path(schema_name: &str) -> PathBuf {
    PathBuf::from(format!("data_{schema_name}.json"))
}

pub async fn write_schema_data(schema_name: &str, records: &[Record]) -> Result<()> {
    write_records(data_file_path(schema_name), records).await
}

pub async fn write_records(path: impl AsRef<Path>, records: &[Record]) -> Result<()> {
    let serialized = serde_json::to_string_pretty(records)?;
    tokio::fs::write(path, serialized).await?;
    Ok(())
}

pub fn process_record(record: HashMap<String, String>, schema: &dsl::Schema) -> Result<Record> {
    let mut output = Record::new();

    for (csv_name, field) in &schema.fields {
        let raw_value = record
            .get(csv_name)
            .ok_or_else(|| format!("unexpected key `{csv_name}` in record"))?;

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
    use crate::dsl::{Field, FieldMeta, FieldType, FieldValue, Schema};
    use std::collections::BTreeMap;
    use tempfile::tempdir;

    fn field(
        ty: FieldType,
        nullable: bool,
        rename: Option<&str>,
        meta: Option<FieldMeta>,
    ) -> Field {
        Field {
            ty,
            nullable,
            rename: rename.map(str::to_string),
            meta,
        }
    }

    fn build_schema(fields: Vec<(&str, Field)>) -> Schema {
        let identifier = fields
            .iter()
            .find(|(_name, field)| field.meta == Some(FieldMeta::Identifier))
            .map(|(name, _)| (*name).to_string())
            .unwrap();

        let mut map = BTreeMap::new();
        for (name, field) in fields {
            map.insert(name.to_string(), field);
        }

        Schema {
            identifier,
            fields: map,
        }
    }

    fn sample_schema() -> Schema {
        build_schema(vec![
            (
                "id",
                field(
                    FieldType::String,
                    false,
                    None,
                    Some(FieldMeta::Identifier),
                ),
            ),
            ("name", field(FieldType::String, false, None, None)),
            ("age", field(FieldType::Int, true, None, None)),
        ])
    }

    #[test]
    fn process_record_ok() {
        let schema = build_schema(vec![
            (
                "id",
                field(
                    FieldType::String,
                    false,
                    None,
                    Some(FieldMeta::Identifier),
                ),
            ),
            (
                "price",
                field(FieldType::Float, true, Some("cost"), None),
            ),
        ]);

        let record = HashMap::from([
            ("id".to_string(), "abc".to_string()),
            ("price".to_string(), "12.5".to_string()),
        ]);

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
                field(
                    FieldType::String,
                    false,
                    None,
                    Some(FieldMeta::Identifier),
                ),
            ),
            ("qty", field(FieldType::Int, true, None, None)),
        ]);

        let record = HashMap::from([
            ("id".to_string(), "1".to_string()),
            ("qty".to_string(), String::new()),
        ]);

        let result = process_record(record, &schema).unwrap();
        assert_eq!(result.get("qty").unwrap(), &None);
    }

    #[test]
    fn process_record_non_nullable_empty_error() {
        let schema = build_schema(vec![
            (
                "id",
                field(
                    FieldType::String,
                    false,
                    None,
                    Some(FieldMeta::Identifier),
                ),
            ),
            ("qty", field(FieldType::Int, false, None, None)),
        ]);

        let record = HashMap::from([
            ("id".to_string(), "123".to_string()),
            ("qty".to_string(), String::new()),
        ]);

        let err = process_record(record, &schema).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("not nullable") && msg.contains("empty value"));
    }

    #[test]
    fn process_record_missing_key_error() {
        let schema = build_schema(vec![(
            "id",
            field(
                FieldType::String,
                false,
                None,
                Some(FieldMeta::Identifier),
            ),
        )]);

        let record = HashMap::new();

        let err = process_record(record, &schema).unwrap_err();
        assert_eq!(err.to_string(), "unexpected key `id` in record");
    }

    #[test]
    fn process_record_ignores_extra_columns() {
        let schema = build_schema(vec![(
            "id",
            field(
                FieldType::String,
                false,
                None,
                Some(FieldMeta::Identifier),
            ),
        )]);

        let record = HashMap::from([
            ("id".to_string(), "abc".to_string()),
            ("ignored".to_string(), "value".to_string()),
        ]);

        let result = process_record(record, &schema).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result.contains_key("id"));
    }

    #[test]
    fn process_record_uses_field_rename() {
        let schema = build_schema(vec![(
            "raw_id",
            field(
                FieldType::String,
                false,
                Some("id"),
                Some(FieldMeta::Identifier),
            ),
        )]);

        let record = HashMap::from([("raw_id".to_string(), "abc".to_string())]);

        let result = process_record(record, &schema).unwrap();
        assert_eq!(
            result.get("id").unwrap(),
            &Some(FieldValue::String("abc".to_string()))
        );
        assert!(!result.contains_key("raw_id"));
    }

    #[test]
    fn process_sheet_content_ok() {
        let schema = sample_schema();
        let csv_data = "id,name,age\n1,Alice,30\n2,Bob,\n3,Charlie,25";

        let result = process_sheet_content(csv_data, &schema).unwrap();

        assert_eq!(result.len(), 3);
        let r0 = &result[0];
        assert_eq!(
            r0.get("name").unwrap(),
            &Some(FieldValue::String("Alice".to_string()))
        );
        assert_eq!(r0.get("age").unwrap(), &Some(FieldValue::Int(30)));
        let r1 = &result[1];
        assert_eq!(
            r1.get("name").unwrap(),
            &Some(FieldValue::String("Bob".to_string()))
        );
        assert_eq!(r1.get("age").unwrap(), &None);
        let r2 = &result[2];
        assert_eq!(
            r2.get("name").unwrap(),
            &Some(FieldValue::String("Charlie".to_string()))
        );
        assert_eq!(r2.get("age").unwrap(), &Some(FieldValue::Int(25)));
    }

    #[test]
    fn process_sheet_content_empty() {
        let schema = sample_schema();

        assert!(process_sheet_content("", &schema).unwrap().is_empty());
        assert!(
            process_sheet_content("id,name,age\n", &schema)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn process_sheet_content_error_on_invalid_type() {
        let schema = build_schema(vec![
            (
                "id",
                field(
                    FieldType::String,
                    false,
                    None,
                    Some(FieldMeta::Identifier),
                ),
            ),
            ("age", field(FieldType::Int, false, None, None)),
        ]);

        let csv_data = "id,age\n1,not_a_number";
        assert!(process_sheet_content(csv_data, &schema).is_err());
    }

    #[test]
    fn data_file_path_uses_schema_name() {
        assert_eq!(
            data_file_path("product"),
            PathBuf::from("data_product.json")
        );
    }

    #[tokio::test]
    async fn write_records_writes_json() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(data_file_path("product"));
        let records = vec![Record::from([
            ("id".to_string(), Some(FieldValue::String("1".to_string()))),
            ("qty".to_string(), None),
        ])];

        write_records(&path, &records).await.unwrap();

        let content = tokio::fs::read_to_string(&path).await.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed, serde_json::json!([{"id": "1", "qty": null}]));
    }
}
