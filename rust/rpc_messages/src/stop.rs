#[cfg(test)]
mod tests {
    use crate::RpcCommand;

    #[test]
    fn serialize_stop_command() {
        assert_eq!(
            serde_json::to_string_pretty(&RpcCommand::Stop).unwrap(),
            r#"{
  "action": "stop"
}"#
        )
    }
}
