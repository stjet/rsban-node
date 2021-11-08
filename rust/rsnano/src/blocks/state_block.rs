use std::{cell::Ref, ops::Deref};

use crate::{
    numbers::{
        from_string_hex, sign_message, to_string_hex, Account, Amount, BlockHash, BlockHashBuilder,
        Link, PublicKey, RawKey, Signature,
    },
    utils::{PropertyTreeReader, PropertyTreeWriter, Stream},
};

#[cfg(test)]
use crate::numbers::KeyPair;

use anyhow::Result;

use super::{BlockType, LazyBlockHash};

#[derive(Clone, PartialEq, Eq, Default, Debug)]
pub struct StateHashables {
    // Account# / public key that operates this account
    // Uses:
    // Bulk signature validation in advance of further ledger processing
    // Arranging uncomitted transactions by account
    pub account: Account,

    // Previous transaction in this chain
    pub previous: BlockHash,

    // Representative of this account
    pub representative: Account,

    // Current balance of this account
    // Allows lookup of account balance simply by looking at the head block
    pub balance: Amount,

    // Link field contains source block_hash if receiving, destination account if sending
    pub link: Link,
}

impl From<&StateHashables> for BlockHash {
    fn from(hashables: &StateHashables) -> Self {
        let mut preamble = [0u8; 32];
        preamble[31] = BlockType::State as u8;
        BlockHashBuilder::new()
            .update(&preamble)
            .update(hashables.account.as_bytes())
            .update(hashables.previous.as_bytes())
            .update(hashables.representative.as_bytes())
            .update(&hashables.balance.to_be_bytes())
            .update(&hashables.link.to_bytes())
            .build()
    }
}

#[derive(Clone, Default, Debug)]
pub struct StateBlock {
    pub work: u64,
    pub signature: Signature,
    pub hashables: StateHashables,
    pub hash: LazyBlockHash,
}

#[allow(clippy::too_many_arguments)]
impl StateBlock {
    pub fn new(
        account: Account,
        previous: BlockHash,
        representative: Account,
        balance: Amount,
        link: Link,
        prv_key: &RawKey,
        pub_key: &PublicKey,
        work: u64,
    ) -> Result<Self> {
        let hashables = StateHashables {
            account,
            previous,
            representative,
            balance,
            link,
        };

        let hash = LazyBlockHash::new();
        let signature = sign_message(prv_key, pub_key, hash.hash(&hashables).as_bytes())?;

        Ok(Self {
            work,
            signature,
            hashables,
            hash,
        })
    }

    fn sign(&mut self, prv_key: &RawKey, pub_key: &PublicKey) -> Result<()> {
        let signature = sign_message(prv_key, pub_key, self.hash().as_bytes())?;
        self.signature = signature;
        Ok(())
    }

    pub const fn serialized_size() -> usize {
        Account::serialized_size() // Account
            + BlockHash::serialized_size() // Previous
            + Account::serialized_size() // Representative
            + Amount::serialized_size() // Balance
            + Link::serialized_size() // Link
            + Signature::serialized_size()
            + std::mem::size_of::<u64>() // Work
    }

    pub fn hash(&'_ self) -> impl Deref<Target=BlockHash> + '_{
        self.hash.hash(&self.hashables)
    }

    pub fn serialize(&self, stream: &mut impl Stream) -> Result<()> {
        self.hashables.account.serialize(stream)?;
        self.hashables.previous.serialize(stream)?;
        self.hashables.representative.serialize(stream)?;
        self.hashables.balance.serialize(stream)?;
        self.hashables.link.serialize(stream)?;
        self.signature.serialize(stream)?;
        stream.write_bytes(&self.work.to_be_bytes())?;
        Ok(())
    }

    pub fn deserialize(stream: &mut impl Stream) -> Result<Self> {
        let account = Account::deserialize(stream)?;
        let previous = BlockHash::deserialize(stream)?;
        let representative = Account::deserialize(stream)?;
        let balance = Amount::deserialize(stream)?;
        let link = Link::deserialize(stream)?;
        let signature = Signature::deserialize(stream)?;
        let mut work_bytes = [0u8; 8];
        stream.read_bytes(&mut work_bytes, 8)?;
        let work = u64::from_be_bytes(work_bytes);
        Ok(Self {
            work,
            signature,
            hashables: StateHashables {
                account,
                previous,
                representative,
                balance,
                link,
            },
            hash: LazyBlockHash::new(),
        })
    }

    pub fn serialize_json(&self, writer: &mut impl PropertyTreeWriter) -> Result<()> {
        writer.put_string("type", "state")?;
        writer.put_string("account", &self.hashables.account.encode_account())?;
        writer.put_string("previous", &self.hashables.previous.encode_hex())?;
        writer.put_string(
            "representative",
            &self.hashables.representative.encode_account(),
        )?;
        writer.put_string("balance", &self.hashables.balance.encode_hex())?;
        writer.put_string("link", &self.hashables.link.encode_hex())?;
        writer.put_string(
            "link_as_account",
            &self.hashables.link.to_account().encode_account(),
        )?;
        writer.put_string("signature", &self.signature.encode_hex())?;
        writer.put_string("work", &to_string_hex(self.work))?;
        Ok(())
    }

    pub fn deserialize_json(reader: &impl PropertyTreeReader) -> Result<Self> {
        let block_type = reader.get_string("type")?;
        if block_type != "state" {
            bail!("invalid block type");
        }
        let account = Account::decode_account(reader.get_string("account")?)?;
        let previous = BlockHash::decode_hex(reader.get_string("previous")?)?;
        let representative = Account::decode_account(reader.get_string("representative")?)?;
        let balance = Amount::decode_hex(reader.get_string("balance")?)?;
        let link = Link::decode_hex(reader.get_string("link")?)?;
        let work = from_string_hex(reader.get_string("work")?)?;
        let signature = Signature::decode_hex(reader.get_string("signature")?)?;
        Ok(StateBlock {
            work,
            signature,
            hashables: StateHashables {
                account,
                previous,
                representative,
                balance,
                link,
            },
            hash: LazyBlockHash::new(),
        })
    }
}

impl PartialEq for StateBlock {
    fn eq(&self, other: &Self) -> bool {
        self.work == other.work
            && self.signature == other.signature
            && self.hashables == other.hashables
    }
}

impl Eq for StateBlock {}

#[cfg(test)]
pub struct StateBlockBuilder {
    block: StateBlock,
    key: KeyPair,
}

#[cfg(test)]
impl StateBlockBuilder {
    pub fn new() -> Self {
        let key = KeyPair::new();
        Self {
            block: StateBlock::new(
                Account::from(1),
                BlockHash::from(2),
                Account::from(3),
                Amount::from(4),
                Link::from(5),
                &key.private_key(),
                &key.public_key(),
                6,
            )
            .unwrap(),
            key,
        }
    }

    pub fn account(mut self, account: impl Into<Account>) -> Self {
        self.block.hashables.account = account.into();
        self
    }

    pub fn previous(mut self, previous: impl Into<BlockHash>) -> Self {
        self.block.hashables.previous = previous.into();
        self
    }

    pub fn representative(mut self, rep: impl Into<Account>) -> Self {
        self.block.hashables.representative = rep.into();
        self
    }

    pub fn balance(mut self, balance: impl Into<Amount>) -> Self {
        self.block.hashables.balance = balance.into();
        self
    }

    pub fn link(mut self, link: impl Into<Link>) -> Self {
        self.block.hashables.link = link.into();
        self
    }

    pub fn sign(mut self, key: KeyPair) -> Self {
        self.key = key;
        self
    }

    pub fn work(mut self, work: u64) -> Self {
        self.block.work = work;
        self
    }

    pub fn build(mut self) -> Result<StateBlock> {
        self.block
            .sign(&self.key.private_key(), &self.key.public_key())?;
        Ok(self.block)
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::{TestPropertyTree, TestStream};

    use super::*;

    // original test: state_block.serialization
    #[test]
    fn builder() {
        let block1 = StateBlockBuilder::new()
            .account(3)
            .previous(1)
            .representative(6)
            .balance(2)
            .link(4)
            .work(5)
            .build()
            .unwrap();

        assert_eq!(block1.hashables.account, Account::from(3));
        assert_eq!(block1.hashables.previous, BlockHash::from(1));
        assert_eq!(block1.hashables.representative, Account::from(6).into());
        assert_eq!(block1.hashables.balance, Amount::new(2));
        assert_eq!(block1.hashables.link, Link::from(4));
    }

    // original test: state_block.serialization
    #[test]
    fn serialization() -> Result<()> {
        let block1 = StateBlockBuilder::new().work(5).build()?;
        let mut stream = TestStream::new();
        block1.serialize(&mut stream)?;
        assert_eq!(StateBlock::serialized_size(), stream.bytes_written());
        assert_eq!(stream.byte_at(215), 0x5); // Ensure work is serialized big-endian

        let block2 = StateBlock::deserialize(&mut stream)?;
        assert_eq!(block1, block2);

        Ok(())
    }

    // original test: state_block.serialization
    #[test]
    fn json_serialization() -> Result<()> {
        let block1 = StateBlockBuilder::new().build()?;

        let mut ptree = TestPropertyTree::new();
        block1.serialize_json(&mut ptree)?;

        let block2 = StateBlock::deserialize_json(&ptree)?;
        assert_eq!(block1, block2);

        Ok(())
    }
}
