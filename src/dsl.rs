// keep-sorted start
use serde::Serialize;
use std::collections::HashMap;
// keep-sorted end

#[derive(Default, Debug)]
pub struct Schema {
    pub fields: HashMap<String, Field>,
}

pub type RawSchema = HashMap<String, String>;

#[derive(Debug)]
pub struct Field {
    pub ty: FieldType,
    pub nullable: bool,
    pub rename: Option<String>,
    pub meta: Option<FieldMeta>,
}

#[derive(Debug, PartialEq)]
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

#[derive(Debug, PartialEq)]
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

#[derive(PartialEq, Debug)]
pub enum FieldMeta {
    Identifier,
    Media(FieldMetaMedia),
}

#[derive(PartialEq, Debug)]
pub enum FieldMetaMedia {
    Image,
    Video,
}

pub fn parse_schema(
    schema: &RawSchema,
) -> Result<Schema, Box<dyn std::error::Error + Send + Sync>> {
    let fields = schema
        .iter()
        .map(|(name, dsl)| parse_field(dsl).map(|field| (name.clone(), field)))
        .collect::<Result<HashMap<String, Field>, _>>()?;

    if !fields
        .values()
        .any(|field| field.meta == Some(FieldMeta::Identifier))
    {
        return Err("no field has been marked as an identifier".into());
    }

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
            if ty != FieldType::String {
                return Err(
                    "expected field type `String` for media metadata, found `{ty:?}`".into(),
                );
            }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_field_basic() {
        let field = parse_field("string").unwrap();
        assert!(matches!(field.ty, FieldType::String));
        assert!(!field.nullable);
        assert_eq!(field.rename, None);
        assert!(field.meta.is_none());

        let field = parse_field("int? -> count @identifier").unwrap();
        assert!(matches!(field.ty, FieldType::Int));
        assert!(field.nullable);
        assert_eq!(field.rename, Some("count".to_owned()));
        assert!(matches!(field.meta, Some(FieldMeta::Identifier)));
    }

    #[test]
    fn parse_field_media() {
        let field = parse_field("string? @media(image)").unwrap();
        assert!(matches!(field.ty, FieldType::String));
        assert!(field.nullable);
        assert!(field.rename.is_none());
        let meta = field.meta.unwrap();
        match meta {
            FieldMeta::Media(media) => assert!(matches!(media, FieldMetaMedia::Image)),
            _ => panic!("expected Media"),
        }
    }

    #[test]
    fn parse_field_media_err() {
        let field_err = parse_field("int @media(image)").unwrap_err();
        let err_msg = field_err.to_string();
        assert_eq!(
            err_msg,
            "expected field type `String` for media metadata, found `{ty:?}`".to_string()
        );
    }

    #[test]
    fn parse_field_errors() {
        assert!(parse_field("unknown").is_err());
        assert!(parse_field("int -> ").is_err()); // empty rename
        assert!(parse_field("string @unknown").is_err());
        assert!(parse_field("string @media(unknown)").is_err());
        assert!(parse_field("bool? extra").is_err()); // unexpected trailing
    }

    #[test]
    fn strip_some_prefix_works() {
        let input = "string? -> name";
        let (prefix, rest) =
            strip_some_prefix(input, vec!["string", "int", "float", "bool"]).unwrap();
        assert_eq!(prefix, "string");
        assert_eq!(rest, "? -> name");

        let input = "float";
        let (prefix, rest) =
            strip_some_prefix(input, vec!["string", "int", "float", "bool"]).unwrap();
        assert_eq!(prefix, "float");
        assert_eq!(rest, "");

        let input = "unknown";
        assert!(strip_some_prefix(input, vec!["string", "int", "float", "bool"]).is_none());
    }

    #[test]
    fn field_type_parse() {
        let ty = FieldType::String;
        assert!(matches!(ty.parse("hello").unwrap(), FieldValue::String(s) if s == "hello"));

        let ty = FieldType::Int;
        assert!(matches!(ty.parse("42").unwrap(), FieldValue::Int(42)));
        assert!(ty.parse("foo").is_err());

        let ty = FieldType::Float;
        assert!(
            matches!(ty.parse("3.14").unwrap(), FieldValue::Float(v) if (v - 3.14).abs() < 1e-9)
        );
        assert!(ty.parse("abc").is_err());

        let ty = FieldType::Bool;
        assert!(matches!(ty.parse("true").unwrap(), FieldValue::Bool(true)));
        assert!(matches!(
            ty.parse("false").unwrap(),
            FieldValue::Bool(false)
        ));
        assert!(matches!(ty.parse("True").unwrap(), FieldValue::Bool(true)));
        assert!(ty.parse("1").is_err());
    }

    #[test]
    fn parse_schema_ok() {
        let mut raw = RawSchema::new();
        raw.insert("id".to_string(), "string @identifier".to_string());
        raw.insert("name".to_string(), "string".to_string());
        raw.insert("qty".to_string(), "int? -> count".to_string());

        let schema = parse_schema(&raw).unwrap();
        assert_eq!(schema.fields.len(), 3);
        let id_field = schema.fields.get("id").unwrap();
        assert!(matches!(id_field.ty, FieldType::String));
        assert!(matches!(id_field.meta, Some(FieldMeta::Identifier)));
        let qty_field = schema.fields.get("qty").unwrap();
        assert!(matches!(qty_field.ty, FieldType::Int));
        assert_eq!(qty_field.rename, Some("count".to_string()));
        assert!(qty_field.nullable);
    }

    #[test]
    fn parse_schema_error() {
        let mut raw = RawSchema::new();
        raw.insert("bad".to_string(), "unknown".to_string());
        assert!(parse_schema(&raw).is_err());
    }

    #[test]
    fn parse_schema_no_identifier() {
        let mut raw = RawSchema::new();
        raw.insert("name".into(), "string".into());
        raw.insert("qty".into(), "int -> count".into());
        let err = parse_schema(&raw).unwrap_err();
        assert!(
            err.to_string()
                .contains("no field has been marked as an identifier")
        )
    }
}
