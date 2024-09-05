use crate::RpcCommand;

impl RpcCommand {
    pub fn representatives() -> Self {
        Self::Representatives
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serialize_representatives_command() {
        let command = RpcCommand::representatives();
        let serialized = serde_json::to_value(command).unwrap();
        let expected = json!({"action": "representatives"});
        assert_eq!(serialized, expected);
    }

    #[test]
    fn deserialize_representatives_command() {
        let json = r#"{"action": "representatives"}"#;
        let deserialized: RpcCommand = serde_json::from_str(json).unwrap();
        assert!(matches!(deserialized, RpcCommand::Representatives));
    }
}

