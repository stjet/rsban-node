use super::Account;
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(Account);

impl NodeId {
    pub fn new(account: Account) -> Self {
        Self(account)
    }

    pub fn encode(&self) -> String {
        let mut node_id = self.0.encode_account();
        node_id.replace_range(0..4, "node");
        node_id
    }

    pub fn decode(source: impl AsRef<str>) -> Result<Self> {
        let mut node_id = source.as_ref().to_string();
        if node_id.starts_with("node_") {
            node_id.replace_range(0..5, "nano_");
            Ok(Self(Account::decode_account(node_id)?))
        } else {
            bail!("Invalid node ID format")
        }
    }

    pub fn to_account(&self) -> Account {
        self.0
    }
}

impl From<Account> for NodeId {
    fn from(account: Account) -> Self {
        Self(account)
    }
}

impl From<NodeId> for Account {
    fn from(node_id: NodeId) -> Self {
        node_id.0
    }
}

impl Serialize for NodeId {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.encode())
    }
}

impl<'de> Deserialize<'de> for NodeId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        NodeId::decode(s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode() {
        let account = Account::decode_account("nano_1y7j5rdqhg99uyab1145gu3yur1ax35a3b6qr417yt8cd6n86uiw3d4whty3").unwrap();
        let node_id = NodeId::new(account);
        let encoded = node_id.encode();
        assert_eq!(encoded, "node_1y7j5rdqhg99uyab1145gu3yur1ax35a3b6qr417yt8cd6n86uiw3d4whty3");
        let decoded = NodeId::decode(&encoded).unwrap();
        assert_eq!(node_id, decoded);
    }

    #[test]
    fn serde_roundtrip() {
        let account = Account::decode_account("nano_1y7j5rdqhg99uyab1145gu3yur1ax35a3b6qr417yt8cd6n86uiw3d4whty3").unwrap();
        let node_id = NodeId::new(account);
        let serialized = serde_json::to_string(&node_id).unwrap();
        let deserialized: NodeId = serde_json::from_str(&serialized).unwrap();
        assert_eq!(node_id, deserialized);
    }

    #[test]
    fn conversion() {
        let account = Account::decode_account("nano_1y7j5rdqhg99uyab1145gu3yur1ax35a3b6qr417yt8cd6n86uiw3d4whty3").unwrap();
        let node_id: NodeId = account.into();
        let back_to_account: Account = node_id.into();
        assert_eq!(account, back_to_account);
    }
}
