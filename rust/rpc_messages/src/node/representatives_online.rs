use crate::RpcCommand;

impl RpcCommand {
    pub fn representatives_online() -> Self {
        Self::RepresentativesOnline
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serialize_representatives_online_command() {
        let command = RpcCommand::representatives_online();
        let serialized = serde_json::to_value(command).unwrap();
        let expected = json!({"action": "representatives_online"});
        assert_eq!(serialized, expected);
    }

    #[test]
    fn deserialize_representatives_online_command() {
        let json = r#"{"action": "representatives_online"}"#;
        let deserialized: RpcCommand = serde_json::from_str(json).unwrap();
        assert!(matches!(deserialized, RpcCommand::RepresentativesOnline));
    }
}