use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct LockedDto {
    pub locked: bool,
}

impl LockedDto {
    pub fn new(locked: bool) -> Self {
        Self { locked }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{from_str, to_string};

    #[test]
    fn serialize_destroyed_dto() {
        let dto = LockedDto::new(true);

        let serialized = to_string(&dto).unwrap();

        let expected_json = serde_json::json!({
            "locked": true
        });

        let actual_json: serde_json::Value = from_str(&serialized).unwrap();
        assert_eq!(actual_json, expected_json);
    }

    #[test]
    fn deserialize_destroyed_dto() {
        let json_str = r#"{
            "locked": true
        }"#;

        let deserialized: LockedDto = from_str(json_str).unwrap();

        let expected = LockedDto::new(true);

        assert_eq!(deserialized, expected);
    }
}
