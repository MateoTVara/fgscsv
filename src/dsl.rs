// keep-sorted start
use serde::Serialize;
use std::collections::HashMap;
// keep-sorted end

pub struct Schema {
    pub fields: HashMap<String, Field>,
}

pub type RawSchema = HashMap<String, String>;

pub struct Field {
    pub ty: FieldType,
    pub nullable: bool,
    pub rename: Option<String>,
    pub meta: Option<FieldMeta>,
}

pub enum FieldType {
    String,
    Int,
    Float,
    Bool,
}

impl FieldType {
    pub fn parse(
        &self,
        value: &str,
    ) -> Result<FieldValue, Box<dyn std::error::Error + Send + Sync>> {
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
pub enum FieldValue {
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

pub fn parse_schema(
    schema: &RawSchema,
) -> Result<Schema, Box<dyn std::error::Error + Send + Sync>> {
    let fields = schema
        .iter()
        .map(|(name, dsl)| parse_field(dsl).map(|field| (name.clone(), field)))
        .collect::<Result<_, _>>()?;

    Ok(Schema { fields })
}

fn parse_field(dsl_command: &str) -> Result<Field, Box<dyn std::error::Error + Send + Sync>> {
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
