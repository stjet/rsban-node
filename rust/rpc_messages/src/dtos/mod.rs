mod account_info;
mod key_pair;

pub use account_info::*;
pub use key_pair::*;
use serde::de::{self, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct SuccessDto {
    success: String,
}

impl SuccessDto {
    pub fn new() -> Self {
        Self {
            success: String::new(),
        }
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ErrorDto {
    error: String,
}

impl ErrorDto {
    pub fn new(error: String) -> Self {
        Self { error }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct BoolMessageDto {
    pub key: String,
    pub value: bool,
}

// Custom implementation for Serialize to format as { "key": value }
impl Serialize for BoolMessageDto {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry(&self.key, &self.value)?;
        map.end()
    }
}

impl<'de> Deserialize<'de> for BoolMessageDto {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct BoolMessageDtoVisitor;

        impl<'de> Visitor<'de> for BoolMessageDtoVisitor {
            type Value = BoolMessageDto;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a map with a single key-value pair where value is a bool")
            }

            fn visit_map<A>(self, mut map: A) -> Result<BoolMessageDto, A::Error>
            where
                A: MapAccess<'de>,
            {
                let (key, value): (String, bool) = match map.next_entry()? {
                    Some(pair) => pair,
                    None => {
                        return Err(de::Error::invalid_length(0, &self));
                    }
                };

                if map.next_entry::<String, bool>()?.is_some() {
                    return Err(de::Error::custom(
                        "Found more than one key-value pair in the map",
                    ));
                }

                Ok(BoolMessageDto { key, value })
            }
        }

        deserializer.deserialize_map(BoolMessageDtoVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{self, from_str};

    #[test]
    fn test_bool_message_dto_serialization() {
        // Create an instance of BoolMessageDto
        let dto = BoolMessageDto {
            key: String::from("test_key"),
            value: true,
        };

        // Serialize the instance to a JSON string
        let serialized = serde_json::to_string(&dto).unwrap();

        // Expected JSON output
        let expected = r#"{"test_key":true}"#;

        // Assert that the serialized output matches the expected output
        assert_eq!(serialized, expected);
    }

    #[test]
    fn test_bool_message_dto_serialization_false_value() {
        // Create an instance of BoolMessageDto with a false value
        let dto = BoolMessageDto {
            key: String::from("another_key"),
            value: false,
        };

        // Serialize the instance to a JSON string
        let serialized = serde_json::to_string(&dto).unwrap();

        // Expected JSON output
        let expected = r#"{"another_key":false}"#;

        // Assert that the serialized output matches the expected output
        assert_eq!(serialized, expected);
    }

    #[test]
    fn test_deserialize_valid() {
        let json_str = r#"{"key1": true}"#;
        let deserialized: BoolMessageDto = from_str(json_str).unwrap();
        assert_eq!(
            deserialized,
            BoolMessageDto {
                key: "key1".to_string(),
                value: true,
            }
        );
    }

    #[test]
    fn test_deserialize_valid_false_value() {
        let json_str = r#"{"key_false": false}"#;
        let deserialized: BoolMessageDto = from_str(json_str).unwrap();
        assert_eq!(
            deserialized,
            BoolMessageDto {
                key: "key_false".to_string(),
                value: false,
            }
        );
    }
}
