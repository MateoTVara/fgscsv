// keep-sorted start
use crate::{
    dsl::{FieldValue, Record},
    Result,
};
use std::{
    collections::BTreeMap,
    fs, io,
    path::Path,
};
// keep-sorted end

pub const STATE_DIR: &str = ".fgscsv";
pub const STATE_FILE_PATH: &str = ".fgscsv/state.json";

#[derive(Debug, PartialEq, Eq)]
pub enum RecordState {
    Added { hash: String },
    Updated { old_hash: String, new_hash: String },
    Deleted { hash: String },
}

// record_id -> state_hash
pub type SheetState = BTreeMap<String, String>;
// schema_name -> record_id -> state_hash
pub type StateMap = BTreeMap<String, SheetState>;

pub fn get_record_states(
    old_state: &SheetState,
    new_state: &SheetState,
) -> BTreeMap<String, RecordState> {
    let mut states = BTreeMap::new();

    for (record_id, new_hash) in new_state {
        match old_state.get(record_id) {
            None => {
                states.insert(
                    record_id.clone(),
                    RecordState::Added {
                        hash: new_hash.clone(),
                    },
                );
            }
            Some(old_hash) if old_hash != new_hash => {
                states.insert(
                    record_id.clone(),
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
    state_path: impl AsRef<Path>,
    schema_name: &str,
) -> Result<SheetState> {
    let state = load_state(state_path)?;
    Ok(state.get(schema_name).cloned().unwrap_or_default())
}

pub fn load_state(state_path: impl AsRef<Path>) -> Result<StateMap> {
    match fs::File::open(state_path) {
        Ok(file) => Ok(serde_json::from_reader(file)?),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(StateMap::new()),
        Err(err) => Err(err.into()),
    }
}

pub fn save_state(state_path: impl AsRef<Path>, state: &StateMap) -> Result<()> {
    let path = state_path.as_ref();

    if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }

    let file = fs::File::create(path)?;
    serde_json::to_writer_pretty(file, state)?;
    Ok(())
}

pub fn get_new_state(records: &[Record], schema_id: &str) -> Result<SheetState> {
    let mut output = SheetState::new();

    for record in records {
        let state_hash = get_record_state_hash(record)?;

        let record_id_value = record
            .get(schema_id)
            .ok_or("identifier field is missing from record")?
            .as_ref()
            .ok_or("identifier field has no value")?;

        let record_id = match record_id_value {
            FieldValue::String(value) => value.clone(),
            FieldValue::Int(value) => value.to_string(),
            FieldValue::Float(value) => value.to_string(),
            FieldValue::Bool(value) => value.to_string(),
        };

        output.insert(record_id, state_hash);
    }

    Ok(output)
}

pub fn get_record_state_hash(record_content: &Record) -> Result<String> {
    let serialized = serde_json::to_vec(record_content)?;
    let hash = blake3::hash(&serialized);
    Ok(hash.to_hex().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn record_with_id(id: FieldValue) -> Record {
        Record::from([("id".to_string(), Some(id))])
    }

    #[test]
    fn get_record_states_detects_changes() {
        let old_state = SheetState::from([
            ("same".to_string(), "hash-same".to_string()),
            ("changed".to_string(), "hash-old".to_string()),
            ("removed".to_string(), "hash-removed".to_string()),
        ]);
        let new_state = SheetState::from([
            ("same".to_string(), "hash-same".to_string()),
            ("changed".to_string(), "hash-new".to_string()),
            ("added".to_string(), "hash-added".to_string()),
        ]);

        let states = get_record_states(&old_state, &new_state);

        assert_eq!(states.len(), 3);
        assert!(!states.contains_key("same"));
        assert_eq!(
            states.get("changed"),
            Some(&RecordState::Updated {
                old_hash: "hash-old".to_string(),
                new_hash: "hash-new".to_string(),
            })
        );
        assert_eq!(
            states.get("added"),
            Some(&RecordState::Added {
                hash: "hash-added".to_string(),
            })
        );
        assert_eq!(
            states.get("removed"),
            Some(&RecordState::Deleted {
                hash: "hash-removed".to_string(),
            })
        );
    }

    #[test]
    fn get_new_state_handles_all_id_types() {
        let records = vec![
            record_with_id(FieldValue::String("abc".to_string())),
            record_with_id(FieldValue::Int(7)),
            record_with_id(FieldValue::Float(1.5)),
            record_with_id(FieldValue::Bool(true)),
        ];

        let state = get_new_state(&records, "id").unwrap();

        assert_eq!(state.len(), 4);
        assert!(state.contains_key("abc"));
        assert!(state.contains_key("7"));
        assert!(state.contains_key("1.5"));
        assert!(state.contains_key("true"));
    }

    #[test]
    fn get_new_state_missing_identifier() {
        let records = vec![Record::from([(
            "name".to_string(),
            Some(FieldValue::String("x".to_string())),
        )])];

        let err = get_new_state(&records, "id").unwrap_err();
        assert_eq!(
            err.to_string(),
            "identifier field is missing from record"
        );
    }

    #[test]
    fn get_new_state_null_identifier() {
        let records = vec![Record::from([("id".to_string(), None)])];

        let err = get_new_state(&records, "id").unwrap_err();
        assert_eq!(err.to_string(), "identifier field has no value");
    }

    #[test]
    fn record_state_hash_is_stable() {
        let record = Record::from([
            ("id".to_string(), Some(FieldValue::String("a".to_string()))),
            ("qty".to_string(), Some(FieldValue::Int(1))),
        ]);

        let first = get_record_state_hash(&record).unwrap();
        let second = get_record_state_hash(&record).unwrap();

        assert_eq!(first, second);
        assert_eq!(first.len(), 64);
    }

    #[test]
    fn record_state_hash_changes_with_content() {
        let record = Record::from([("id".to_string(), Some(FieldValue::String("a".to_string())))]);
        let mut changed = record.clone();
        changed.insert("id".to_string(), Some(FieldValue::String("b".to_string())));

        assert_ne!(
            get_record_state_hash(&record).unwrap(),
            get_record_state_hash(&changed).unwrap()
        );
    }

    #[test]
    fn state_save_and_load_round_trip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(STATE_DIR).join("state.json");
        let state = StateMap::from([(
            "product".to_string(),
            SheetState::from([("id-1".to_string(), "hash-1".to_string())]),
        )]);

        save_state(&path, &state).unwrap();

        assert!(path.exists());
        assert_eq!(load_state(&path).unwrap(), state);
    }

    #[test]
    fn load_state_missing_file_is_empty() {
        let dir = tempdir().unwrap();

        let state = load_state(dir.path().join("missing.json")).unwrap();

        assert!(state.is_empty());
    }

    #[test]
    fn get_current_state_reads_matching_schema() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("state.json");
        let state = StateMap::from([(
            "product".to_string(),
            SheetState::from([("id-1".to_string(), "hash-1".to_string())]),
        )]);
        save_state(&path, &state).unwrap();

        let current = get_current_state(&path, "product").unwrap();
        assert_eq!(current.len(), 1);
        assert_eq!(current["id-1"], "hash-1");

        let missing = get_current_state(&path, "category").unwrap();
        assert!(missing.is_empty());
    }
}
