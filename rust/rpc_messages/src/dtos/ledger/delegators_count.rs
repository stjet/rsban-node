use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct CountRpcMessage {
    pub count: u64,
}

impl CountRpcMessage {
    pub fn new(count: u64) -> Self {
        Self { count }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serialize_count_dto() {
        let count_dto = CountRpcMessage::new(42);
        let serialized = serde_json::to_value(count_dto).unwrap();
        let expected = json!({"count": 42});
        assert_eq!(serialized, expected);
    }

    #[test]
    fn deserialize_count_dto() {
        let json = r#"{"count": 42}"#;
        let deserialized: CountRpcMessage = serde_json::from_str(json).unwrap();
        assert_eq!(deserialized, CountRpcMessage::new(42));
    }
}
