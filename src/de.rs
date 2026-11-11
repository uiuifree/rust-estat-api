//! Deserializers for the quirks of e-Stat's XML to JSON conversion.
//!
//! * A repeated element becomes an array, but a single one becomes an object.
//! * Element text that looks like a number becomes a JSON number
//!   (`"SURVEY_DATE": 201606`), otherwise a string.
//! * An empty element with child elements in the schema becomes `""`.

use serde::de::{DeserializeOwned, Error};
use serde::{Deserialize, Deserializer};
use serde_json::Value;

/// One object or an array of them, as a `Vec`. `null` is an empty `Vec`.
pub fn one_or_many<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::Null => Ok(Vec::new()),
        Value::Array(_) => serde_json::from_value(value).map_err(D::Error::custom),
        _ => serde_json::from_value(value)
            .map(|one| vec![one])
            .map_err(D::Error::custom),
    }
}

fn to_text<E: Error>(value: Value) -> Result<String, E> {
    match value {
        Value::String(s) => Ok(s),
        Value::Number(n) => Ok(n.to_string()),
        other => Err(E::custom(format!("expected a string, got {other}"))),
    }
}

/// String or number as a `String`.
pub fn text<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    to_text(Value::deserialize(deserializer)?)
}

/// String or number as a `String`, `null` as `None`.
pub fn opt_text<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<String>, D::Error> {
    match Value::deserialize(deserializer)? {
        Value::Null => Ok(None),
        value => to_text(value).map(Some),
    }
}

fn to_number<T: TryFrom<u64>, E: Error>(value: &Value) -> Result<T, E> {
    let n = match value {
        Value::Number(n) => n.as_u64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    };
    n.and_then(|n| T::try_from(n).ok())
        .ok_or_else(|| E::custom(format!("expected a count, got {value}")))
}

/// Number or numeric string.
pub fn number<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: TryFrom<u64>,
{
    to_number(&Value::deserialize(deserializer)?)
}

/// Number or numeric string, `null` or `""` as `None`.
pub fn opt_number<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: TryFrom<u64>,
{
    match Value::deserialize(deserializer)? {
        Value::Null => Ok(None),
        Value::String(s) if s.is_empty() => Ok(None),
        value => to_number(&value).map(Some),
    }
}

/// An object, or `None` for the `""` (or `null`) that an empty element becomes.
pub fn object_or_empty<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    match Value::deserialize(deserializer)? {
        Value::Null | Value::String(_) => Ok(None),
        value => serde_json::from_value(value)
            .map(Some)
            .map_err(D::Error::custom),
    }
}

/// An object, or the default for the `""` (or `null`) that an empty element becomes.
pub fn object_or_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned + Default,
{
    object_or_empty(deserializer).map(Option::unwrap_or_default)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[derive(Debug, Deserialize, PartialEq)]
    struct Item {
        id: u8,
    }

    #[derive(Debug, Default, Deserialize, PartialEq)]
    struct Holder2 {
        #[serde(default)]
        n: u8,
    }

    #[derive(Debug, Deserialize)]
    struct Holder {
        #[serde(default, deserialize_with = "one_or_many")]
        items: Vec<Item>,
        #[serde(default, deserialize_with = "opt_text")]
        text: Option<String>,
        #[serde(default, deserialize_with = "opt_number")]
        count: Option<u32>,
        #[serde(default, deserialize_with = "object_or_empty")]
        object: Option<Item>,
        #[serde(default, deserialize_with = "object_or_default")]
        list: Holder2,
    }

    fn holder(value: Value) -> Result<Holder, serde_json::Error> {
        serde_json::from_value(value)
    }

    #[test]
    fn single_object_or_array() {
        let one = holder(json!({"items": {"id": 1}})).unwrap();
        assert_eq!(one.items, [Item { id: 1 }]);
        let many = holder(json!({"items": [{"id": 1}, {"id": 2}]})).unwrap();
        assert_eq!(many.items.len(), 2);
        assert!(holder(json!({"items": null})).unwrap().items.is_empty());
        assert!(holder(json!({})).unwrap().items.is_empty());
        assert!(holder(json!({"items": {"id": "x"}})).is_err());
        assert!(holder(json!({"items": [{"id": "x"}]})).is_err());
    }

    #[test]
    fn text_from_string_or_number() {
        assert_eq!(holder(json!({"text": "a"})).unwrap().text.unwrap(), "a");
        assert_eq!(
            holder(json!({"text": 201606})).unwrap().text.unwrap(),
            "201606"
        );
        assert_eq!(holder(json!({"text": null})).unwrap().text, None);
        let error = holder(json!({"text": [1]})).unwrap_err();
        assert!(error.to_string().contains("expected a string"), "{error}");

        let text = |value: Value| text(value).map_err(|e: serde_json::Error| e.to_string());
        assert_eq!(text(json!(1.5)).unwrap(), "1.5");
        assert!(text(json!(true)).is_err());
    }

    #[test]
    fn numbers_from_number_or_string() {
        assert_eq!(holder(json!({"count": 3})).unwrap().count, Some(3));
        assert_eq!(holder(json!({"count": " 42 "})).unwrap().count, Some(42));
        assert_eq!(holder(json!({"count": ""})).unwrap().count, None);
        assert_eq!(holder(json!({"count": null})).unwrap().count, None);
        for bad in [
            json!("x"),
            json!(-1),
            json!(1.5),
            json!(5_000_000_000u64),
            json!({}),
        ] {
            let error = holder(json!({ "count": bad })).unwrap_err();
            assert!(error.to_string().contains("expected a count"), "{error}");
        }
        let status: u16 = number(json!("100")).unwrap();
        assert_eq!(status, 100);
        assert!(number::<_, u16>(json!(70000)).is_err());
    }

    #[test]
    fn empty_string_for_missing_objects() {
        assert_eq!(holder(json!({"object": ""})).unwrap().object, None);
        assert_eq!(holder(json!({"object": null})).unwrap().object, None);
        assert_eq!(
            holder(json!({"object": {"id": 7}})).unwrap().object,
            Some(Item { id: 7 })
        );
        assert!(holder(json!({"object": {"id": "x"}})).is_err());

        assert_eq!(
            holder(json!({"list": ""})).unwrap().list,
            Holder2::default()
        );
        assert_eq!(holder(json!({"list": {"n": 2}})).unwrap().list.n, 2);
        assert!(holder(json!({"list": {"n": "x"}})).is_err());
    }
}
