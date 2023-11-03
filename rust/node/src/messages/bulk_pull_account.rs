use super::{MessageHeader, MessageType};
use anyhow::Result;
use num_traits::FromPrimitive;
use rsnano_core::{
    utils::{Deserialize, FixedSizeSerialize, Serialize, Stream},
    Account, Amount,
};
use std::{fmt::Display, mem::size_of};

#[derive(Clone, Copy, PartialEq, Eq, FromPrimitive, Debug)]
#[repr(u8)]
pub enum BulkPullAccountFlags {
    PendingHashAndAmount = 0x0,
    PendingAddressOnly = 0x1,
    PendingHashAmountAndAddress = 0x2,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BulkPullAccountPayload {
    pub account: Account,
    pub minimum_amount: Amount,
    pub flags: BulkPullAccountFlags,
}

impl BulkPullAccountPayload {
    pub fn deserialize(stream: &mut impl Stream, header: &MessageHeader) -> Result<Self> {
        debug_assert!(header.message_type == MessageType::BulkPullAccount);
        let payload = Self {
            account: Account::deserialize(stream)?,
            minimum_amount: Amount::deserialize(stream)?,
            flags: BulkPullAccountFlags::from_u8(stream.read_u8()?)
                .ok_or_else(|| anyhow!("invalid bulk pull account flag"))?,
        };
        Ok(payload)
    }

    pub fn serialized_size() -> usize {
        Account::serialized_size() + Amount::serialized_size() + size_of::<BulkPullAccountFlags>()
    }

    pub fn create_test_instance() -> BulkPullAccountPayload {
        Self {
            account: 1.into(),
            minimum_amount: 42.into(),
            flags: BulkPullAccountFlags::PendingHashAndAmount,
        }
    }
}

impl Serialize for BulkPullAccountPayload {
    fn serialize(&self, stream: &mut dyn Stream) -> Result<()> {
        self.account.serialize(stream)?;
        self.minimum_amount.serialize(stream)?;
        stream.write_u8(self.flags as u8)
    }
}

impl Display for BulkPullAccountPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "\nacc={} min={}",
            self.account.encode_hex(),
            self.minimum_amount.to_string_dec()
        )?;

        let flag_str = match self.flags {
            BulkPullAccountFlags::PendingHashAndAmount => "pending_hash_and_amount",
            BulkPullAccountFlags::PendingAddressOnly => "pending_address_only",
            BulkPullAccountFlags::PendingHashAmountAndAddress => "pending_hash_amount_and_address",
        };

        write!(f, " {}", flag_str)
    }
}
