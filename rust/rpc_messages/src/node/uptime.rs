use crate::RpcCommand;
use serde::{Deserialize, Serialize};

impl RpcCommand {
    pub fn uptime() -> Self {
        Self::Uptime
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct UptimeDto {
    pub seconds: u64,
}

impl UptimeDto {
    pub fn new(seconds: u64) -> Self {
        Self { seconds }
    }
}

#[cfg(test)]
mod tests {
    use crate::{RpcCommand, UptimeDto};
    use serde_json::{from_str, to_string_pretty};

    #[test]
    fn deserialize_uptime_dto() {
        let block_count_dto = UptimeDto::new(1);
        let serialized = to_string_pretty(&block_count_dto).unwrap();
        let deserialized: UptimeDto = from_str(&serialized).unwrap();
        assert_eq!(block_count_dto, deserialized);
    }

    #[test]
    fn serialize_uptime_dto() {
        assert_eq!(
            serde_json::to_string_pretty(&UptimeDto::new(1)).unwrap(),
            r#"{
  "seconds": 1
}"#
        )
    }

    #[test]
    fn serialize_uptime_command() {
        assert_eq!(
            to_string_pretty(&RpcCommand::Uptime).unwrap(),
            r#"{
  "action": "uptime"
}"#
        );
    }

    #[test]
    fn deserialize_uptime_command() {
        let cmd = RpcCommand::Uptime;
        let serialized = to_string_pretty(&cmd).unwrap();
        let deserialized: RpcCommand = from_str(&serialized).unwrap();
        assert_eq!(cmd, deserialized);
    }
}
