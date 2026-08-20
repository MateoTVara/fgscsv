// keep-sorted start
use crate::dsl;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs};
// keep-sorted end

pub const CONFIG_FILE_PATH: &str = "fgscsv.toml";

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub root_path: String,
    pub schemas: HashMap<String, dsl::RawSchema>,
    pub sheets: HashMap<String, Sheet>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Sheet {
    pub name: String,
    pub schema: String,
}

pub fn get_config() -> Result<Config, Box<dyn std::error::Error + Send + Sync>> {
    let file_content = fs::read_to_string(CONFIG_FILE_PATH)?;
    let config: Config = toml::from_str(&file_content)?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use tempfile::tempdir;

    #[test]
    fn get_config_ok() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join(CONFIG_FILE_PATH);

        let content = r#"
            root_path = "http://example.com/"
            [schemas.product]
            id = "string @identifier"
            name = "string"
            [sheets]
            "1234" = { name = "Products", schema = "product" }
        "#;
        std::fs::write(&file_path, content).unwrap();

        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(dir.path()).unwrap();

        let config = get_config().unwrap();
        assert_eq!(config.root_path, "http://example.com/");
        assert!(config.schemas.contains_key("product"));
        assert!(config.sheets.contains_key("1234"));

        env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn get_config_file_not_found() {
        let dir = tempdir().unwrap();
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(dir.path()).unwrap();

        let result = get_config();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("No such file") || err.to_string().contains("cannot find")
        );

        env::set_current_dir(original_dir).unwrap();
    }
}
