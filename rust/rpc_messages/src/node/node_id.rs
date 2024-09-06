use rsnano_core::{Account, PublicKey, RawKey};
use serde::{Deserialize, Serialize, Serializer, Deserializer};
use crate::RpcCommand;

impl RpcCommand {
    pub fn node_id() -> Self {
        Self::NodeId
    }
}

#[derive(PartialEq, Eq, Debug)]
pub struct NodeIdDto {
    pub private: RawKey,
    pub public: PublicKey,
    pub as_account: Account,
    pub node_id: Account,
}

impl NodeIdDto {
    pub fn new(private: RawKey, public: PublicKey, as_account: Account) -> Self {
        Self { private, public, as_account, node_id: as_account }
    }
}

impl Serialize for NodeIdDto {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("NodeIdDto", 4)?;
        state.serialize_field("private", &self.private)?;
        state.serialize_field("public", &self.public)?;
        state.serialize_field("as_account", &self.as_account)?;
        state.serialize_field("node_id", &self.node_id.to_node_id())?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for NodeIdDto {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct NodeIdDtoHelper {
            private: RawKey,
            public: PublicKey,
            as_account: Account,
            node_id: String,
        }

        let helper = NodeIdDtoHelper::deserialize(deserializer)?;
        let account_str = helper.node_id.replacen("node", "nano", 1);

        Ok(NodeIdDto {
            private: helper.private,
            public: helper.public,
            as_account: helper.as_account,
            node_id: Account::decode_account(&account_str).map_err(serde::de::Error::custom)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use super::*;

    #[test]
    fn serialize_node_id_dto() {
        let node_id_dto = NodeIdDto {
            private: RawKey::zero(),
            public: PublicKey::zero(),
            as_account: Account::zero(),
            node_id: Account::zero(),
        };

        let serialized = serde_json::to_value(&node_id_dto).unwrap();
        let expected = json!({
            "private": "0000000000000000000000000000000000000000000000000000000000000000",
            "public": "0000000000000000000000000000000000000000000000000000000000000000",
            "as_account": "nano_1111111111111111111111111111111111111111111111111111hifc8npp",
            "node_id": "node_1111111111111111111111111111111111111111111111111111hifc8npp"
        });

        assert_eq!(serialized, expected);
    }

    #[test]
    fn deserialize_node_id_dto() {
        let json_str = r#"{
            "private": "0000000000000000000000000000000000000000000000000000000000000000",
            "public": "0000000000000000000000000000000000000000000000000000000000000000",
            "as_account": "nano_1111111111111111111111111111111111111111111111111111hifc8npp",
            "node_id": "node_1111111111111111111111111111111111111111111111111111hifc8npp"
        }"#;

        let deserialized: NodeIdDto = serde_json::from_str(json_str).unwrap();

        let node_id_dto = NodeIdDto {
            private: RawKey::zero(),
            public: PublicKey::zero(),
            as_account: Account::zero(),
            node_id: Account::zero(),
        };

        assert_eq!(deserialized, node_id_dto);
    }

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
