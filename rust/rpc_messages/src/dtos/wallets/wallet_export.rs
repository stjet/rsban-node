use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct JsonDto {
    pub json: Value,
}

impl JsonDto {
    pub fn new(json: Value) -> Self {
        Self { json }
    }
}

#[cfg(test)]
mod tests {
    use super::JsonDto;
    use serde_json::Value;

    #[test]
    fn serialize_json_dto() {
        let json = Value::Object(Default::default());
        let json_dto = JsonDto::new(json);
        let serialized = serde_json::to_string(&json_dto).unwrap();

        let expected_serialized = r#"{"json":{}}"#;

        assert_eq!(serialized, expected_serialized);
    }

    #[test]
    fn deserialize_json_dto() {
        let json_str = r#"{"json":{}}"#;

        let deserialized: JsonDto = serde_json::from_str(json_str).unwrap();

        let expected = JsonDto::new(Value::Object(Default::default()));

        assert_eq!(deserialized, expected);
    }
}
