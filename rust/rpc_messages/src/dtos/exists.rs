use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ExistsDto {
    pub exists: bool,
}

impl ExistsDto {
    pub fn new(exists: bool) -> Self {
        Self { exists }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{from_str, to_string};

    #[test]
    fn serialize_account_balance_dto() {
        let dto = ExistsDto::new(true);

        let serialized = to_string(&dto).unwrap();

        let expected_json = serde_json::json!({
            "exists": true
        });

        let actual_json: serde_json::Value = from_str(&serialized).unwrap();
        assert_eq!(actual_json, expected_json);
    }

    #[test]
    fn deserialize_account_balance_dto() {
        let json_str = r#"{
            "exists": true
        }"#;

        let deserialized: ExistsDto = from_str(json_str).unwrap();

        let expected = ExistsDto::new(true);

        assert_eq!(deserialized, expected);
    }
}
