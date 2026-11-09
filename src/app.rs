// keep-sorted start
use crate::{
    config::{self, Config},
    dsl::{self, Record, Schema},
    pipeline, state, Result,
};
use std::{
    collections::BTreeMap,
    fs,
    io::{self, BufRead, Write},
    path::Path,
};
// keep-sorted end

pub async fn run() -> Result<()> {
    let config = config::get_config()?;
    run_with_config(&config).await
}

async fn run_with_config(config: &Config) -> Result<()> {
    let sheets = prepare_sheets(config)?;
    let results = fetch_all(&config.root_path, sheets).await?;
    let (records_per_schema, new_state_per_schema) = merge_by_schema(results);

    let current_state = state::load_state(state::STATE_FILE_PATH)?;
    report_changes(&current_state, &new_state_per_schema);

    write_outputs(&records_per_schema).await?;
    state::save_state(state::STATE_FILE_PATH, &new_state_per_schema)?;

    Ok(())
}

fn prepare_sheets(config: &Config) -> Result<Vec<(String, String, Schema)>> {
    config
        .sheets
        .iter()
        .map(|(sheet_id, sheet)| {
            let raw_schema = config
                .schemas
                .get(&sheet.schema)
                .ok_or_else(|| format!("schema `{}` doesn't exist", sheet.schema))?;
            let schema = dsl::parse_schema(raw_schema)?;
            Ok((sheet_id.clone(), sheet.schema.clone(), schema))
        })
        .collect()
}

async fn fetch_all(
    root_path: &str,
    sheets: Vec<(String, String, Schema)>,
) -> Result<Vec<(String, pipeline::SheetResult)>> {
    let mut handles = Vec::with_capacity(sheets.len());

    for (sheet_id, schema_name, schema) in sheets {
        let root_path = root_path.to_owned();
        handles.push((
            schema_name,
            tokio::spawn(pipeline::process_sheet(root_path, sheet_id, schema)),
        ));
    }

    let mut results = Vec::with_capacity(handles.len());
    for (schema_name, handle) in handles {
        results.push((schema_name, handle.await??));
    }

    Ok(results)
}

fn merge_by_schema(
    results: Vec<(String, pipeline::SheetResult)>,
) -> (BTreeMap<String, Vec<Record>>, state::StateMap) {
    let mut records_per_schema: BTreeMap<String, Vec<Record>> = BTreeMap::new();
    let mut new_state_per_schema = state::StateMap::new();

    for (schema_name, sheet) in results {
        records_per_schema
            .entry(schema_name.clone())
            .or_default()
            .extend(sheet.records);
        new_state_per_schema
            .entry(schema_name)
            .or_default()
            .extend(sheet.state);
    }

    (records_per_schema, new_state_per_schema)
}

fn report_changes(current: &state::StateMap, new: &state::StateMap) {
    for (schema_name, new_state) in new {
        let old_state = current.get(schema_name).cloned().unwrap_or_default();
        let states = state::get_record_states(&old_state, new_state);

        for (record_id, change) in states {
            match change {
                state::RecordState::Added { hash } => {
                    println!("{record_id} was added: {hash}");
                }
                state::RecordState::Updated { old_hash, new_hash } => {
                    println!("{record_id} was updated");
                    println!("  old: {old_hash}");
                    println!("  new: {new_hash}");
                }
                state::RecordState::Deleted { hash } => {
                    println!("{record_id} was deleted: {hash}");
                }
            }
        }
    }
}

async fn write_outputs(records_per_schema: &BTreeMap<String, Vec<Record>>) -> Result<()> {
    for (schema_name, records) in records_per_schema {
        pipeline::write_schema_data(schema_name, records).await?;
    }

    Ok(())
}

pub fn init() -> Result<()> {
    if fs::exists(config::CONFIG_FILE_PATH)?
        && !confirm_overwrite(io::stdin().lock(), io::stdout().lock())?
    {
        println!("Exiting...");
        return Ok(());
    }

    create_initial_config(config::CONFIG_FILE_PATH)
}

pub fn create_initial_config(path: impl AsRef<Path>) -> Result<()> {
    config::save_config(path, &config::initial_config())
}

fn confirm_overwrite<R: BufRead, W: Write>(mut reader: R, mut writer: W) -> Result<bool> {
    loop {
        write!(
            writer,
            "Existing {} file, do you want to overwrite the file? [y/N] ",
            config::CONFIG_FILE_PATH
        )?;
        writer.flush()?;

        let mut answer = String::new();
        if reader.read_line(&mut answer)? == 0 {
            return Ok(false);
        }

        match answer.trim().to_lowercase().as_str() {
            "y" | "yes" => return Ok(true),
            "n" | "no" | "" => return Ok(false),
            _ => writeln!(writer, "Please enter y or n.")?,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        dsl::FieldValue,
        pipeline::SheetResult,
        state::SheetState,
    };
    use std::io::Cursor;
    use tempfile::tempdir;

    fn sample_config() -> Config {
        Config {
            root_path: "http://example.com/".to_string(),
            schemas: BTreeMap::from([(
                "product".to_string(),
                BTreeMap::from([
                    ("id".to_string(), "string @identifier".to_string()),
                    ("name".to_string(), "string".to_string()),
                ]),
            )]),
            sheets: BTreeMap::from([(
                "1234".to_string(),
                config::Sheet {
                    name: "Products".to_string(),
                    schema: "product".to_string(),
                },
            )]),
        }
    }

    fn sheet_result(id: &str) -> SheetResult {
        SheetResult {
            records: vec![Record::from([(
                "id".to_string(),
                Some(FieldValue::String(id.to_string())),
            )])],
            state: SheetState::from([(id.to_string(), format!("hash-{id}"))]),
        }
    }

    #[test]
    fn prepare_sheets_parses_schema() {
        let sheets = prepare_sheets(&sample_config()).unwrap();

        assert_eq!(sheets.len(), 1);
        assert_eq!(sheets[0].0, "1234");
        assert_eq!(sheets[0].1, "product");
        assert_eq!(sheets[0].2.identifier, "id");
        assert_eq!(sheets[0].2.fields.len(), 2);
    }

    #[test]
    fn prepare_sheets_missing_schema_errors() {
        let mut config = sample_config();
        config.sheets.get_mut("1234").unwrap().schema = "missing".to_string();

        let err = prepare_sheets(&config).unwrap_err();
        assert_eq!(err.to_string(), "schema `missing` doesn't exist");
    }

    #[test]
    fn merge_by_schema_combines_sheets() {
        let (records, states) = merge_by_schema(vec![
            ("product".to_string(), sheet_result("1")),
            ("product".to_string(), sheet_result("2")),
            ("category".to_string(), sheet_result("3")),
        ]);

        assert_eq!(records["product"].len(), 2);
        assert_eq!(records["category"].len(), 1);
        assert_eq!(states["product"].len(), 2);
        assert_eq!(states["category"].len(), 1);
        assert_eq!(states["product"]["1"], "hash-1");
    }

    fn prompt(input: &str) -> (bool, String) {
        let mut output = Vec::new();
        let answer = confirm_overwrite(Cursor::new(input), &mut output).unwrap();
        (answer, String::from_utf8(output).unwrap())
    }

    #[test]
    fn confirm_overwrite_accepts_yes() {
        assert!(prompt("y\n").0);
        assert!(prompt("YES\n").0);
    }

    #[test]
    fn confirm_overwrite_rejects_no_and_blank() {
        assert!(!prompt("n\n").0);
        assert!(!prompt("no\n").0);
        assert!(!prompt("\n").0);
        assert!(!prompt("").0);
    }

    #[test]
    fn confirm_overwrite_retries_on_invalid_input() {
        let (answer, output) = prompt("maybe\ny\n");

        assert!(answer);
        assert!(output.contains("Please enter y or n."));
    }

    #[test]
    fn create_initial_config_writes_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("fgscsv.toml");

        create_initial_config(&path).unwrap();

        let loaded = config::load_config(&path).unwrap();
        assert_eq!(loaded, config::initial_config());
    }
}
