use crate::{
    utils::{BufferWriter, Deserialize, FixedSizeSerialize, Serialize, Stream},
    Account, Block, BlockEnum, BlockHash,
};
use primitive_types::U512;

#[derive(Default, PartialEq, Eq, Debug, Clone)]
pub struct PendingKey {
    pub account: Account,
    pub hash: BlockHash,
}

impl PendingKey {
    pub fn new(account: Account, hash: BlockHash) -> Self {
        Self { account, hash }
    }

    pub fn to_bytes(&self) -> [u8; 64] {
        let mut result = [0; 64];
        result[..32].copy_from_slice(self.account.as_bytes());
        result[32..].copy_from_slice(self.hash.as_bytes());
        result
    }

    pub fn for_send_block(block: &BlockEnum) -> Self {
        Self::new(block.link_field().unwrap_or_default().into(), block.hash())
    }

    pub fn for_receive_block(block: &BlockEnum) -> Self {
        Self::new(
            block.account(),
            block.link_field().unwrap_or_default().into(),
        )
    }

    pub fn create_test_instance() -> Self {
        Self::new(Account::from(1), BlockHash::from(2))
    }
}

impl Serialize for PendingKey {
    fn serialize(&self, writer: &mut dyn BufferWriter) {
        self.account.serialize(writer);
        self.hash.serialize(writer);
    }
}

impl FixedSizeSerialize for PendingKey {
    fn serialized_size() -> usize {
        Account::serialized_size() + BlockHash::serialized_size()
    }
}

impl Deserialize for PendingKey {
    type Target = Self;

    fn deserialize(stream: &mut dyn Stream) -> anyhow::Result<Self::Target> {
        let account = Account::deserialize(stream)?;
        let hash = BlockHash::deserialize(stream)?;
        Ok(Self { account, hash })
    }
}

impl From<U512> for PendingKey {
    fn from(value: U512) -> Self {
        let mut buffer = [0; 64];
        value.to_big_endian(&mut buffer);
        PendingKey::new(
            Account::from_slice(&buffer[..32]).unwrap(),
            BlockHash::from_slice(&buffer[32..]).unwrap(),
        )
    }
}

impl PartialOrd for PendingKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        if self.account == other.account {
            self.hash.partial_cmp(&other.hash)
        } else {
            self.account.partial_cmp(&other.account)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::PendingKey;

    #[test]
    fn pending_key_sorting() {
        let one = PendingKey::new(1.into(), 2.into());
        let one_same = PendingKey::new(1.into(), 2.into());
        let two = PendingKey::new(1.into(), 3.into());
        let three = PendingKey::new(2.into(), 1.into());
        assert!(one < two);
        assert!(one < three);
        assert!(two < three);
        assert!(one == one_same);
        assert!(one != two);
    }
}
