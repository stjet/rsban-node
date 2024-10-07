use crate::{AccountRpcMessage, RpcCommand};
use rsnano_core::Account;

impl RpcCommand {
    pub fn validate_account_number(account: Account) -> Self {
        Self::ValidateAccountNumber(AccountRpcMessage::new("account".to_string(), account))
    }
}

#[cfg(test)]
mod tests {
    use crate::RpcCommand;
    use serde_json::to_string_pretty;
    use std::{net::Ipv6Addr, str::FromStr};

    #[test]
    fn serialize_validate_account_number_command() {
        assert_eq!(
            to_string_pretty(&RpcCommand::Keepalive(AddressWithPortArg::new(
                Ipv6Addr::from_str("::ffff:192.169.0.1").unwrap(),
                1024
            )))
            .unwrap(),
            r#"{
  "action": "validate_account_number",
  "account": "::ffff:192.169.0.1"
}"#
        )
    }

    #[test]
    fn deserialize_validate_account_number_command() {
        let json_str = r#"{
"action": "keepalive",
"address": "::ffff:192.169.0.1",
"port": 1024
}"#;
        let deserialized: RpcCommand = serde_json::from_str(json_str).unwrap();
        let expected_command =
            RpcCommand::keepalive(Ipv6Addr::from_str("::ffff:192.169.0.1").unwrap(), 1024);
        assert_eq!(deserialized, expected_command);
    }
}
