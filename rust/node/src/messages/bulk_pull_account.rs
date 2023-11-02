use super::{Message, MessageHeader, MessageType, MessageVisitor, ProtocolInfo};
use anyhow::Result;
use num_traits::FromPrimitive;
use rsnano_core::{
    utils::{Deserialize, FixedSizeSerialize, Serialize, Stream},
    Account, Amount,
};
use std::{any::Any, fmt::Display, mem::size_of};

#[derive(Clone, Copy, PartialEq, Eq, FromPrimitive, Debug)]
#[repr(u8)]
pub enum BulkPullAccountFlags {
    PendingHashAndAmount = 0x0,
    PendingAddressOnly = 0x1,
    PendingHashAmountAndAddress = 0x2,
}

#[derive(Clone)]
pub struct BulkPullAccount {
    header: MessageHeader,
    pub account: Account,
    pub minimum_amount: Amount,
    pub flags: BulkPullAccountFlags,
}

impl BulkPullAccount {
    pub fn new(protocol_info: &ProtocolInfo) -> Self {
        Self {
            header: MessageHeader::new(MessageType::BulkPullAccount, protocol_info),
            account: Account::zero(),
            minimum_amount: Amount::zero(),
            flags: BulkPullAccountFlags::PendingHashAndAmount,
        }
    }

    pub fn with_header(header: MessageHeader) -> Self {
        Self {
            header,
            account: Account::zero(),
            minimum_amount: Amount::zero(),
            flags: BulkPullAccountFlags::PendingHashAndAmount,
        }
    }

    pub fn from_stream(stream: &mut impl Stream, header: MessageHeader) -> Result<Self> {
        let mut msg = Self::with_header(header);
        msg.deserialize(stream)?;
        Ok(msg)
    }

    pub fn serialized_size() -> usize {
        Account::serialized_size() + Amount::serialized_size() + size_of::<BulkPullAccountFlags>()
    }

    pub fn deserialize(&mut self, stream: &mut impl Stream) -> Result<()> {
        debug_assert!(self.header.message_type == MessageType::BulkPullAccount);
        self.account = Account::deserialize(stream)?;
        self.minimum_amount = Amount::deserialize(stream)?;
        self.flags = BulkPullAccountFlags::from_u8(stream.read_u8()?)
            .ok_or_else(|| anyhow!("invalid bulk pull account flag"))?;
        Ok(())
    }
}

impl Message for BulkPullAccount {
    fn header(&self) -> &MessageHeader {
        &self.header
    }

    fn set_header(&mut self, header: &MessageHeader) {
        self.header = header.clone();
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn serialize(&self, stream: &mut dyn Stream) -> Result<()> {
        self.header.serialize(stream)?;
        self.account.serialize(stream)?;
        self.minimum_amount.serialize(stream)?;
        stream.write_u8(self.flags as u8)
    }

    fn visit(&self, visitor: &mut dyn MessageVisitor) {
        visitor.bulk_pull_account(self)
    }

    fn clone_box(&self) -> Box<dyn Message> {
        Box::new(self.clone())
    }

    fn message_type(&self) -> MessageType {
        MessageType::BulkPullAccount
    }
}

impl Display for BulkPullAccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.header.fmt(f)?;
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
