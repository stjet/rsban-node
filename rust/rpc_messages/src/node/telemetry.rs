use std::net::Ipv6Addr;
use crate::RpcCommand;
use rsnano_messages::TelemetryData;
use serde::{ser::SerializeMap, Deserialize, Deserializer, Serialize, Serializer};

impl RpcCommand {
    pub fn telemetry(raw: Option<bool>, address: Option<Ipv6Addr>, port: Option<u16>) -> Self {
        Self::Telemetry(TelemetryArgs::new(raw, address, port))
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct TelemetryArgs {
    pub raw: Option<bool>,
    pub address: Option<Ipv6Addr>,
    pub port: Option<u16>
}

impl TelemetryArgs {
    pub fn new(raw: Option<bool>, address: Option<Ipv6Addr>, port: Option<u16>) -> Self {
        Self {
            raw,
            address,
            port,
        }
    }
}

#[derive(PartialEq, Eq, Debug)]
pub struct TelemetryDto {
    pub metrics: Vec<TelemetryData>
}

impl Serialize for TelemetryDto {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if self.metrics.len() == 1 {
            self.metrics[0].serialize(serializer)
        } else {
            let mut map = serializer.serialize_map(Some(1))?;
            map.serialize_entry("metrics", &self.metrics)?;
            map.end()
        }
    }
}

impl<'de> Deserialize<'de> for TelemetryDto {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum TelemetryDtoHelper {
            Single(TelemetryData),
            Multiple { metrics: Vec<TelemetryData> },
        }

        let helper = TelemetryDtoHelper::deserialize(deserializer)?;
        match helper {
            TelemetryDtoHelper::Single(data) => Ok(TelemetryDto { metrics: vec![data] }),
            TelemetryDtoHelper::Multiple { metrics } => Ok(TelemetryDto { metrics }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsnano_core::{BlockHash, PublicKey, Signature};
    use rsnano_messages::TelemetryData;
    use std::time::UNIX_EPOCH;

    fn create_test_telemetry_data() -> TelemetryData {
        TelemetryData {
            signature: Signature::new(),
            node_id: PublicKey::zero(),
            block_count: 1000,
            cemented_count: 900,
            unchecked_count: 100,
            account_count: 500,
            bandwidth_cap: 1024,
            uptime: 3600,
            peer_count: 10,
            protocol_version: 18,
            genesis_block: BlockHash::zero(),
            major_version: 23,
            minor_version: 3,
            patch_version: 0,
            pre_release_version: 0,
            maker: 3,
            timestamp: UNIX_EPOCH + std::time::Duration::from_secs(1623456789),
            active_difficulty: 0xFFFFFFFFFFFFFFFF,
            unknown_data: vec![],
        }
    }

    #[test]
    fn test_telemetry_dto_serialize_single() {
        let data = create_test_telemetry_data();
        let dto = TelemetryDto { metrics: vec![data.clone()] };
        
        let serialized = serde_json::to_string(&dto).unwrap();
        let deserialized: TelemetryData = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(data, deserialized);
    }

    #[test]
    fn test_telemetry_dto_serialize_multiple() {
        let data1 = create_test_telemetry_data();
        let mut data2 = data1.clone();
        data2.block_count = 2000;
        
        let dto = TelemetryDto { metrics: vec![data1.clone(), data2.clone()] };
        
        let serialized = serde_json::to_string(&dto).unwrap();
        let deserialized: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        
        assert!(deserialized.is_object());
        assert!(deserialized.get("metrics").is_some());
        
        let metrics = deserialized["metrics"].as_array().unwrap();
        assert_eq!(metrics.len(), 2);
        
        let deserialized_data1: TelemetryData = serde_json::from_value(metrics[0].clone()).unwrap();
        let deserialized_data2: TelemetryData = serde_json::from_value(metrics[1].clone()).unwrap();
        
        assert_eq!(data1, deserialized_data1);
        assert_eq!(data2, deserialized_data2);
    }

    #[test]
    fn test_telemetry_dto_deserialize_single() {
        let data = create_test_telemetry_data();
        let json = serde_json::to_string(&data).unwrap();
        
        let deserialized: TelemetryDto = serde_json::from_str(&json).unwrap();
        
        assert_eq!(deserialized.metrics.len(), 1);
        assert_eq!(data, deserialized.metrics[0]);
    }

    #[test]
    fn test_telemetry_dto_deserialize_multiple() {
        let data1 = create_test_telemetry_data();
        let mut data2 = data1.clone();
        data2.block_count = 2000;
        
        let json = format!(
            r#"{{"metrics":[{},{}]}}"#,
            serde_json::to_string(&data1).unwrap(),
            serde_json::to_string(&data2).unwrap()
        );
        
        let deserialized: TelemetryDto = serde_json::from_str(&json).unwrap();
        
        assert_eq!(deserialized.metrics.len(), 2);
        assert_eq!(data1, deserialized.metrics[0]);
        assert_eq!(data2, deserialized.metrics[1]);
    }
}
