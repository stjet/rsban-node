use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct RemovedDto {
    pub removed: bool,
}

impl RemovedDto {
    pub fn new(removed: bool) -> Self {
        Self { removed }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{from_str, to_string};

    #[test]
    fn serialize_destroyed_dto() {
        let dto = RemovedDto::new(true);

        let serialized = to_string(&dto).unwrap();

        let expected_json = serde_json::json!({
            "removed": true
        });

        let actual_json: serde_json::Value = from_str(&serialized).unwrap();
        assert_eq!(actual_json, expected_json);
    }

    #[test]
    fn deserialize_destroyed_dto() {
        let json_str = r#"{
            "removed": true
        }"#;

        let deserialized: RemovedDto = from_str(json_str).unwrap();

        let expected = RemovedDto::new(true);

        assert_eq!(deserialized, expected);
    }
}
