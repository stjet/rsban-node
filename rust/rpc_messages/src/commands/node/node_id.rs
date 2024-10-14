use crate::RpcCommand;

impl RpcCommand {
    pub fn node_id() -> Self {
        Self::NodeId
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serialize_node_id_command() {
        let command = RpcCommand::node_id();
        let serialized = serde_json::to_value(&command).unwrap();
        let expected = json!({
            "action": "node_id"
        });
        assert_eq!(serialized, expected);
    }

    #[test]
    fn deserialize_node_id_command() {
        let json_str = r#"{"action": "node_id"}"#;
        let deserialized: RpcCommand = serde_json::from_str(json_str).unwrap();
        assert!(matches!(deserialized, RpcCommand::NodeId));
    }
}
