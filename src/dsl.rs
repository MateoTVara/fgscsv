// keep-sorted start
use crate::Result;
use serde::Serialize;
use std::collections::BTreeMap;
// keep-sorted end

#[derive(Clone, Debug, Default)]
pub struct Schema {
    pub fields: BTreeMap<String, Field>,
    pub identifier: String,
}

pub type RawSchema = BTreeMap<String, String>;

pub type Record = BTreeMap<String, Option<FieldValue>>;

#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub ty: FieldType,
    pub nullable: bool,
    pub rename: Option<String>,
    pub meta: Option<FieldMeta>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldType {
    String,
    Int,
    Float,
    Bool,
}

impl FieldType {
    pub fn parse(&self, value: &str) -> Result<FieldValue> {
        let val = match self {
            Self::String => FieldValue::String(value.to_owned()),
            Self::Int => FieldValue::Int(value.parse()?),
            Self::Float => FieldValue::Float(value.parse()?),
            Self::Bool => {
                let boolean_val = match value.to_lowercase().as_str() {
                    "true" => true,
                    "false" => false,
                    _ => return Err(format!("invalid bool value `{value}`").into()),
                };
                FieldValue::Bool(boolean_val)
            }
        };

        Ok(val)
    }

    fn parse_prefix(input: &str) -> Option<(Self, &str)> {
        const TYPES: [(&str, FieldType); 4] = [
            ("string", FieldType::String),
            ("int", FieldType::Int),
            ("float", FieldType::Float),
            ("bool", FieldType::Bool),
        ];

        TYPES
            .into_iter()
            .find_map(|(prefix, ty)| input.strip_prefix(prefix).map(|rest| (ty, rest)))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum FieldValue {
    String(String),
    Int(u32),
    Float(f64),
    Bool(bool),
}

impl Serialize for FieldValue {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FieldMeta {
    Identifier,
    Media(FieldMetaMedia),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FieldMetaMedia {
    Image,
    Video,
}

pub fn parse_schema(schema: &RawSchema) -> Result<Schema> {
    let fields = schema
        .iter()
        .map(|(name, dsl)| parse_field(dsl).map(|field| (name.clone(), field)))
        .collect::<Result<BTreeMap<String, Field>>>()?;

    let identifier = {
        let mut identifiers = fields
            .iter()
            .filter(|(_name, field)| field.meta == Some(FieldMeta::Identifier));

        let (name, field) = identifiers
            .next()
            .ok_or("no field has been marked as an identifier")?;

        if identifiers.next().is_some() {
            return Err("more than one field has been marked as an identifier".into());
        }

        field.rename.clone().unwrap_or_else(|| name.clone())
    };

    Ok(Schema { identifier, fields })
}

fn parse_field(dsl_command: &str) -> Result<Field> {
    let (ty, rest) = FieldType::parse_prefix(dsl_command).ok_or("unexpected type")?;

    let (nullable, rest) = match rest.strip_prefix('?') {
        Some(rest) => (true, rest),
        None => (false, rest),
    };

    let (rename, rest) = parse_rename(rest)?;
    let meta = parse_meta(rest, ty)?;

    Ok(Field {
        ty,
        nullable,
        rename,
        meta,
    })
}

fn parse_rename(rest: &str) -> Result<(Option<String>, &str)> {
    let Some(after_arrow) = rest.strip_prefix(" -> ") else {
        return Ok((None, rest));
    };

    let end_index = after_arrow.find(' ').unwrap_or(after_arrow.len());
    let new_name = &after_arrow[..end_index];

    if new_name.is_empty() {
        return Err("expected identifier after ` -> `".into());
    }

    Ok((Some(new_name.to_owned()), &after_arrow[end_index..]))
}

fn parse_meta(rest: &str, ty: FieldType) -> Result<Option<FieldMeta>> {
    if rest.is_empty() {
        return Ok(None);
    }

    let Some(rest) = rest.strip_prefix(' ') else {
        return Err(format!("unexpected input `{rest}`").into());
    };

    if rest == "@identifier" {
        return Ok(Some(FieldMeta::Identifier));
    }

    if let Some(media) = rest.strip_prefix("@media(") {
        if ty != FieldType::String {
            return Err(
                format!("expected field type `String` for media metadata, found `{ty:?}`").into(),
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

        return Ok(Some(FieldMeta::Media(media)));
    }

    Err(format!("unknown metadata `{rest}`").into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_field_all_types() {
        for (input, expected) in [
            ("string", FieldType::String),
            ("int", FieldType::Int),
            ("float", FieldType::Float),
            ("bool", FieldType::Bool),
        ] {
            let field = parse_field(input).unwrap();
            assert_eq!(field.ty, expected);
            assert!(!field.nullable);
            assert!(field.rename.is_none());
            assert!(field.meta.is_none());
        }
    }

    #[test]
    fn parse_field_basic() {
        let field = parse_field("string").unwrap();
        assert_eq!(field.ty, FieldType::String);
        assert!(!field.nullable);
        assert_eq!(field.rename, None);
        assert!(field.meta.is_none());

        let field = parse_field("int? -> count @identifier").unwrap();
        assert_eq!(field.ty, FieldType::Int);
        assert!(field.nullable);
        assert_eq!(field.rename, Some("count".to_owned()));
        assert_eq!(field.meta, Some(FieldMeta::Identifier));
    }

    #[test]
    fn parse_field_media() {
        let field = parse_field("string? @media(image)").unwrap();
        assert_eq!(field.ty, FieldType::String);
        assert!(field.nullable);
        assert!(field.rename.is_none());
        assert_eq!(
            field.meta,
            Some(FieldMeta::Media(FieldMetaMedia::Image))
        );

        let field = parse_field("string @media(video)").unwrap();
        assert_eq!(
            field.meta,
            Some(FieldMeta::Media(FieldMetaMedia::Video))
        );
    }

    #[test]
    fn parse_field_media_err() {
        let err = parse_field("int @media(image)").unwrap_err();
        assert_eq!(
            err.to_string(),
            "expected field type `String` for media metadata, found `Int`"
        );
    }

    #[test]
    fn parse_field_media_missing_paren() {
        let err = parse_field("string @media(image").unwrap_err();
        assert_eq!(err.to_string(), "expected `)` after media type");
    }

    #[test]
    fn parse_field_errors() {
        assert_eq!(parse_field("unknown").unwrap_err().to_string(), "unexpected type");
        assert_eq!(
            parse_field("int -> ").unwrap_err().to_string(),
            "expected identifier after ` -> `"
        );
        assert_eq!(
            parse_field("string @unknown").unwrap_err().to_string(),
            "unknown metadata `@unknown`"
        );
        assert_eq!(
            parse_field("string @media(unknown)")
                .unwrap_err()
                .to_string(),
            "unknown media type `unknown`"
        );
        assert_eq!(
            parse_field("bool? extra").unwrap_err().to_string(),
            "unknown metadata `extra`"
        );
        assert_eq!(
            parse_field("stringy").unwrap_err().to_string(),
            "unexpected input `y`"
        );
    }

    #[test]
    fn parse_field_type_prefix() {
        assert_eq!(
            FieldType::parse_prefix("string? -> name"),
            Some((FieldType::String, "? -> name"))
        );
        assert_eq!(FieldType::parse_prefix("float"), Some((FieldType::Float, "")));
        assert_eq!(FieldType::parse_prefix("unknown"), None);
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
        assert!(matches!(
            ty.parse("FALSE").unwrap(),
            FieldValue::Bool(false)
        ));
        assert!(ty.parse("1").is_err());
    }

    #[test]
    fn field_value_serializes_json() {
        assert_eq!(
            serde_json::to_value(FieldValue::String("hello".into())).unwrap(),
            serde_json::json!("hello")
        );
        assert_eq!(
            serde_json::to_value(FieldValue::Int(42)).unwrap(),
            serde_json::json!(42)
        );
        assert_eq!(
            serde_json::to_value(FieldValue::Float(3.5)).unwrap(),
            serde_json::json!(3.5)
        );
        assert_eq!(
            serde_json::to_value(FieldValue::Bool(true)).unwrap(),
            serde_json::json!(true)
        );
    }

    #[test]
    fn parse_schema_ok() {
        let mut raw = RawSchema::new();
        raw.insert("id".to_string(), "string @identifier".to_string());
        raw.insert("name".to_string(), "string".to_string());
        raw.insert("qty".to_string(), "int? -> count".to_string());

        let schema = parse_schema(&raw).unwrap();
        assert_eq!(schema.identifier, "id");
        assert_eq!(schema.fields.len(), 3);
        let id_field = schema.fields.get("id").unwrap();
        assert_eq!(id_field.ty, FieldType::String);
        assert_eq!(id_field.meta, Some(FieldMeta::Identifier));
        let qty_field = schema.fields.get("qty").unwrap();
        assert_eq!(qty_field.ty, FieldType::Int);
        assert_eq!(qty_field.rename, Some("count".to_string()));
        assert!(qty_field.nullable);
    }

    #[test]
    fn parse_schema_identifier_uses_rename() {
        let mut raw = RawSchema::new();
        raw.insert("id".to_string(), "string -> product_id @identifier".to_string());

        let schema = parse_schema(&raw).unwrap();
        assert_eq!(schema.identifier, "product_id");
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
        assert_eq!(
            err.to_string(),
            "no field has been marked as an identifier"
        );
    }

    #[test]
    fn parse_schema_multiple_identifiers() {
        let mut raw = RawSchema::new();
        raw.insert("a".into(), "string @identifier".into());
        raw.insert("b".into(), "int @identifier".into());

        let err = parse_schema(&raw).unwrap_err();
        assert_eq!(
            err.to_string(),
            "more than one field has been marked as an identifier"
        );
    }
}
