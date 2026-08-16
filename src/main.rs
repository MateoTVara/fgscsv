// keep-sorted start
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    io::{self, Read, Write},
};
// keep-sorted end

#[derive(Subcommand)]
enum Commands {
    /// Fetch and process spreadsheet data from the web according to the configuration.
    Run,

    /// Create an initial configuration file.
    Init,
}

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Serialize, Deserialize)]
struct Config {
    root_path: String,
    schemas: HashMap<String, HashMap<String, String>>,
    sheets: HashMap<String, Sheet>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Sheet {
    name: String,
    schema: String,
}

const CONFIG_FILE_PATH: &str = "fgscsv.toml";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run => run_cli().await?,
        Commands::Init => init_cli()?,
    };

    Ok(())
}

fn get_config() -> Result<Config, Box<dyn std::error::Error + Send + Sync>> {
    let mut file = fs::File::open(CONFIG_FILE_PATH)?;
    let mut file_content = String::new();
    file.read_to_string(&mut file_content)?;

    let config: Config = toml::from_str(&file_content)?;
    Ok(config)
}

// #################################
// ######## Run ####################
// #################################

async fn run_cli() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = get_config()?;

    let handles = config.sheets.iter().map(|(sheet_id, sheet)| {
        tokio::spawn(process_sheet(
            config.root_path.clone(),
            sheet_id.clone(),
            sheet.clone(),
        ))
    });

    for handle in handles {
        handle.await??;
    }

    Ok(())
}

// #################################
// ####### Sheets ##################
// #################################

async fn process_sheet(
    root_path: String,
    sheet_id: String,
    sheet: Sheet,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let sheet_content = fetch_sheet(&root_path, &sheet_id).await?;
    let serialized = serialize_sheet(&sheet_content, &sheet)?;
    write_sheet_data(&sheet_id, &serialized).await?;
    Ok(())
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

fn serialize_sheet(
    sheet_content: &str,
    sheet: &Sheet,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let mut rdr = csv::Reader::from_reader(sheet_content.as_bytes());
    // let records: Vec<TempStruct> = rdr
    //     .deserialize::<TempStruct>()
    //     .collect::<Result<_, csv::Error>>()?;
    let mut records: Vec<HashMap<String, Option<FieldValue>>> = Vec::new();
    for result in rdr.deserialize::<HashMap<String, String>>() {
        let record = result?;
        let record = deserialize_record(record, &sheet.schema)?;
        records.push(record);
    }
    let serialized = serde_json::to_string_pretty(&records)?;
    println!("{}", serialized);
    Ok(serialized)
}

async fn write_sheet_data(
    sheet_id: &str,
    sheet_data: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tokio::fs::write(format!("data_{sheet_id}.json"), sheet_data).await?;
    Ok(())
}

// #################################
// ######## Init ###################
// #################################

fn init_cli() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if fs::exists(CONFIG_FILE_PATH)? && !confirm_overwrite()? {
        println!("Exiting...");
        return Ok(());
    }

    create_initial_config()?;

    Ok(())
}

fn create_initial_config() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let initial_config = Config {
        root_path: String::from("url/to/your/csv/file"),
        schemas: HashMap::from([(
            "product".to_string(),
            HashMap::from([
                ("id".to_string(), "string @identifier".to_string()),
                ("name".to_string(), "string".to_string()),
                ("price".to_string(), "float".to_string()),
                ("base qty".to_string(), "string -> base_qty".to_string()),
                ("description".to_string(), "string?".to_string()),
                ("img1".to_string(), "string? @media(image)".to_string()),
            ]),
        )]),
        sheets: HashMap::from([
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
    };
    let serialized_initial_config = toml::to_string(&initial_config)?;
    fs::write(CONFIG_FILE_PATH, serialized_initial_config)?;
    Ok(())
}

fn confirm_overwrite() -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    loop {
        print!(
            "Existing {} file, do you want to overwrite the file? [y/N] ",
            CONFIG_FILE_PATH
        );
        io::stdout().flush()?;

        let mut answer = String::new();
        io::stdin().read_line(&mut answer)?;

        match answer.trim().to_lowercase().as_str() {
            "y" | "yes" => return Ok(true),
            "n" | "no" | "" => return Ok(false),
            _ => println!("Please enter y or n."),
        }
    }
}

// #################################
// ###### Parser ###################
// #################################

struct Field {
    ty: FieldType,
    nullable: bool,
    rename: Option<String>,
    meta: Option<FieldMeta>,
}

enum FieldType {
    String,
    Int,
    Float,
    Bool,
}

impl FieldType {
    fn parse(&self, value: &str) -> Result<FieldValue, Box<dyn std::error::Error + Send + Sync>> {
        let val = match self {
            Self::String => FieldValue::String(value.to_owned()),
            Self::Int => FieldValue::Int(value.parse()?),
            Self::Float => FieldValue::Float(value.parse()?),
            Self::Bool => {
                let boolean_val = match value.to_lowercase().as_str() {
                    "true" => true,
                    "false" => false,
                    _ => return Err(format!("invalid bool value {value}").into()),
                };
                FieldValue::Bool(boolean_val)
            }
        };

        Ok(val)
    }
}

#[derive(Debug)]
enum FieldValue {
    String(String),
    Int(u32),
    Float(f64),
    Bool(bool),
}

impl Serialize for FieldValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            FieldValue::String(v) => serializer.serialize_str(v),
            FieldValue::Int(v) => serializer.serialize_u32(*v),
            FieldValue::Float(v) => serializer.serialize_f64(*v),
            FieldValue::Bool(v) => serializer.serialize_bool(*v),
        }
    }
}

enum FieldMeta {
    Identifier,
    Media(FieldMetaMedia),
}

enum FieldMetaMedia {
    Image,
    Video,
}

fn deserialize_record(
    record: HashMap<String, String>,
    schema_name: &str,
) -> Result<HashMap<String, Option<FieldValue>>, Box<dyn std::error::Error + Send + Sync>> {
    let config = get_config()?;
    let schema_fields = match config.schemas.get(schema_name) {
        Some(val) => val,
        None => {
            return Err(format!("No schema with name {schema_name} exists.").into());
        }
    };

    let mut record_to_return: HashMap<String, Option<FieldValue>> = HashMap::new();
    for (csv_name, dsl_command) in schema_fields.into_iter() {
        let field = deserialize_field(dsl_command)?;
        let unproccesed_record_val = record.get(csv_name).ok_or("unexpected key in record")?;

        let val = if unproccesed_record_val.is_empty() {
            if field.nullable {
                None
            } else {
                return Err(
                    format!("field `{csv_name}` is not nullable but has an empty value").into(),
                );
            }
        } else {
            Some(field.ty.parse(unproccesed_record_val)?)
        };

        if let Some(new_name) = field.rename {
            record_to_return.insert(new_name, val);
        } else {
            record_to_return.insert(csv_name.to_owned(), val);
        }
    }

    Ok(record_to_return)
}

fn deserialize_field(dsl_command: &str) -> Result<Field, Box<dyn std::error::Error + Send + Sync>> {
    let mut rest = dsl_command;

    let (ty, remaining) = match strip_some_prefix(rest, vec!["string", "int", "float", "bool"]) {
        Some(("string", rem)) => (FieldType::String, rem),
        Some(("int", rem)) => (FieldType::Int, rem),
        Some(("float", rem)) => (FieldType::Float, rem),
        Some(("bool", rem)) => (FieldType::Bool, rem),
        _ => return Err("unexpected type".into()),
    };
    rest = remaining;

    let nullable = match rest.strip_prefix('?') {
        Some(_) => {
            rest = &rest[1..];
            true
        }
        None => false,
    };

    let rename = if let Some(after_arrow) = rest.strip_prefix(" -> ") {
        let end_index = after_arrow.find(' ').unwrap_or(after_arrow.len());
        let new_name = &after_arrow[..end_index];

        if new_name.is_empty() {
            return Err("Expected identifier after ` -> `".into());
        }

        rest = &after_arrow[end_index..];

        Some(new_name.to_owned())
    } else {
        None
    };

    let meta = if rest.is_empty() {
        None
    } else if let Some(after_space) = rest.strip_prefix(' ') {
        rest = after_space;

        if rest == "@identifier" {
            Some(FieldMeta::Identifier)
        } else if let Some(media) = rest.strip_prefix("@media(") {
            let media = media
                .strip_suffix(')')
                .ok_or("expected `)` after media type")?;

            let media = match media {
                "image" => FieldMetaMedia::Image,
                "video" => FieldMetaMedia::Video,
                _ => return Err(format!("unknown media type `{media}`").into()),
            };

            Some(FieldMeta::Media(media))
        } else {
            return Err(format!("unknown metadata `{rest}`").into());
        }
    } else {
        return Err(format!("unexpected input `{rest}`").into());
    };

    Ok(Field {
        ty,
        nullable,
        rename,
        meta,
    })
}

fn strip_some_prefix<'a>(
    input: &'a str,
    possible_prefixes: Vec<&'a str>,
) -> Option<(&'a str, &'a str)> {
    for prefix in possible_prefixes {
        let val = input.strip_prefix(prefix);
        if val != None {
            return Some((prefix, val.unwrap()));
        }
    }
    None
}
