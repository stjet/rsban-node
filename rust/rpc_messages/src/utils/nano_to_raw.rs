use crate::RpcCommand;
use rsnano_core::Amount;
use serde::{Deserialize, Serialize};

impl RpcCommand {
    pub fn nano_to_raw(nano: Amount) -> Self {
        Self::NanoToRaw(NanoToRawArgs::new(nano))
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct NanoToRawArgs {
    pub amount: Amount,
}

impl NanoToRawArgs {
    pub fn new(amount: Amount) -> Self {
        Self { amount }
    }
}

#[cfg(test)]
mod tests {
    use crate::RpcCommand;
    use rsnano_core::Amount;
    use serde_json::{from_str, to_string_pretty};

    #[test]
    fn serialize_nano_to_raw_command() {
        assert_eq!(
            to_string_pretty(&RpcCommand::nano_to_raw(Amount::nano(1))).unwrap(),
            r#"{
  "action": "nano_to_raw",
  "amount": "1000000000000000000000000000000"
}"#
        );
    }

    #[test]
    fn deserialize_nano_to_raw_command() {
        let cmd = RpcCommand::nano_to_raw(Amount::nano(1));
        let serialized = to_string_pretty(&cmd).unwrap();
        let deserialized: RpcCommand = from_str(&serialized).unwrap();
        assert_eq!(cmd, deserialized);
    }
}
