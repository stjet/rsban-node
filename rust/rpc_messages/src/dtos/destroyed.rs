use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct DestroyedDto {
    pub destroyed: bool,
}

impl DestroyedDto {
    pub fn new(destroyed: bool) -> Self {
        Self { destroyed }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{from_str, to_string};

    #[test]
    fn serialize_destroyed_dto() {
        let dto = DestroyedDto::new(true);

        let serialized = to_string(&dto).unwrap();

        let expected_json = serde_json::json!({
            "destroyed": true
        });

        let actual_json: serde_json::Value = from_str(&serialized).unwrap();
        assert_eq!(actual_json, expected_json);
    }

    #[test]
    fn deserialize_destroyed_dto() {
        let json_str = r#"{
            "destroyed": true
        }"#;

        let deserialized: DestroyedDto = from_str(json_str).unwrap();

        let expected = DestroyedDto::new(true);

        assert_eq!(deserialized, expected);
    }
}
