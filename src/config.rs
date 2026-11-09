// keep-sorted start
use crate::{dsl, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path};
// keep-sorted end

pub const CONFIG_FILE_PATH: &str = "fgscsv.toml";

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub root_path: String,
    pub schemas: BTreeMap<String, dsl::RawSchema>,
    pub sheets: BTreeMap<String, Sheet>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sheet {
    pub name: String,
    pub schema: String,
}

pub fn get_config() -> Result<Config> {
    load_config(CONFIG_FILE_PATH)
}

pub fn load_config(path: impl AsRef<Path>) -> Result<Config> {
    let file_content = fs::read_to_string(path)?;
    let config: Config = toml::from_str(&file_content)?;
    Ok(config)
}

pub fn save_config(path: impl AsRef<Path>, config: &Config) -> Result<()> {
    let serialized = toml::to_string(config)?;
    fs::write(path, serialized)?;
    Ok(())
}

pub fn initial_config() -> Config {
    Config {
        root_path: String::from("url/to/your/csv/file"),
        schemas: BTreeMap::from([(
            "product".to_string(),
            BTreeMap::from([
                ("id".to_string(), "string @identifier".to_string()),
                ("name".to_string(), "string".to_string()),
                ("price".to_string(), "float".to_string()),
                ("base qty".to_string(), "string -> base_qty".to_string()),
                ("description".to_string(), "string?".to_string()),
                ("img1".to_string(), "string? @media(image)".to_string()),
            ]),
        )]),
        sheets: BTreeMap::from([
            (
                "1234".to_string(),
                Sheet {
                    name: "Products".to_string(),
                    schema: "product".to_string(),
                },
            ),
            (
                "4321".to_string(),
                Sheet {
                    name: "Categories".to_string(),
                    schema: "product".to_string(),
                },
            ),
        ]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn sample_config_toml() -> &'static str {
        r#"
            root_path = "http://example.com/"
            [schemas.product]
            id = "string @identifier"
            name = "string"
            [sheets]
            "1234" = { name = "Products", schema = "product" }
        "#
    }

    #[test]
    fn load_config_ok() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(CONFIG_FILE_PATH);
        fs::write(&path, sample_config_toml()).unwrap();

        let config = load_config(&path).unwrap();
        assert_eq!(config.root_path, "http://example.com/");
        assert!(config.schemas.contains_key("product"));
        assert!(config.sheets.contains_key("1234"));
    }

    #[test]
    fn load_config_file_not_found() {
        let dir = tempdir().unwrap();
        let result = load_config(dir.path().join("missing.toml"));

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("No such file") || err.to_string().contains("cannot find")
        );
    }

    #[test]
    fn load_config_invalid_toml() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(CONFIG_FILE_PATH);
        fs::write(&path, "root_path = [").unwrap();

        assert!(load_config(&path).is_err());
    }

    #[test]
    fn save_config_round_trips() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(CONFIG_FILE_PATH);
        let config = initial_config();

        save_config(&path, &config).unwrap();
        let loaded = load_config(&path).unwrap();

        assert_eq!(loaded, config);
    }

    #[test]
    fn initial_config_has_expected_shape() {
        let config = initial_config();

        assert!(!config.root_path.is_empty());
        assert!(config.schemas.contains_key("product"));
        assert_eq!(config.sheets.len(), 2);
        assert_eq!(config.sheets["1234"].schema, "product");
    }
}
