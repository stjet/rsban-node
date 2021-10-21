use primitive_types::U256;

use crate::{block_details::BlockDetails, epoch::Epoch};

pub struct PublicKey {
    value: U256,
}

impl PublicKey {
    pub fn new(value: U256) -> Self {
        Self { value }
    }

    pub fn serialized_size() -> usize {
        32
    }
}

pub struct Account {
    public_key: PublicKey,
}

impl Account {
    pub fn new(public_key: PublicKey) -> Self {
        Self { public_key }
    }

    pub fn serialized_size() -> usize {
        PublicKey::serialized_size()
    }
}

pub struct BlockHash {
    value: U256,
}

impl BlockHash {
    pub fn new(value: U256) -> Self {
        Self { value }
    }

    pub fn serialized_size() -> usize{
       32 
    }
}

pub struct Amount {
    value: u128,
}

impl Amount {
    pub fn new(value: u128) -> Self {
        Self { value }
    }

    pub fn serialized_size() -> usize {
        std::mem::size_of::<u128>()
    }
}

#[repr(u8)]
#[derive(PartialEq, Eq, Debug, Clone, Copy, FromPrimitive)]
pub enum BlockType {
    Invalid = 0,
    NotABlock = 1,
    Send = 2,
    Receive = 3,
    Open = 4,
    Change = 5,
    State = 6,
}

pub struct BlockSideband {
    height: u64,
    timestamp: u64,
    successor: BlockHash,
    account: Account,
    balance: Amount,
    details: BlockDetails,
    source_epoch: Epoch,
}

impl BlockSideband {
    pub fn new(
        account: Account,
        successor: BlockHash,
        balance: Amount,
        height: u64,
        timestamp: u64,
        details: BlockDetails,
        source_epoch: Epoch,
    ) -> Self {
        Self {
            height,
            timestamp,
            successor,
            account,
            balance,
            details,
            source_epoch,
        }
    }

    pub fn serialized_size(block_type: BlockType) -> usize {
        let mut size = BlockHash::serialized_size(); // successor

        if block_type != BlockType::State && block_type != BlockType::Open {
            size += Account::serialized_size(); // account
        }

        if block_type != BlockType::Open {
            size += std::mem::size_of::<u64>(); // height
        }

        if block_type == BlockType::Receive || block_type == BlockType::Change || block_type == BlockType::Open {
            size += Amount::serialized_size(); // balance
        }

        size += std::mem::size_of::<u64>(); // timestamp

        if block_type == BlockType::State {
            // block_details must not be larger than the epoch enum
            const_assert!(std::mem::size_of::<Epoch>() == BlockDetails::serialized_size());
            size += BlockDetails::serialized_size() + std::mem::size_of::<Epoch>();
        }

        size
    }
}
