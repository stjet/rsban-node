use std::cell::{Ref, RefCell};

use crate::{numbers::{sign_message, Account, Amount, BlockHash, PublicKey, RawKey, Signature}, utils::{Blake2b, PropertyTreeWriter, RustBlake2b, Stream}};
use anyhow::Result;

use super::BlockType;

#[derive(Clone, PartialEq, Eq, Default, Debug)]
pub struct SendHashables {
    pub previous: BlockHash,
    pub destination: Account,
    pub balance: Amount,
}

impl SendHashables {
    pub const fn serialized_size() -> usize {
        BlockHash::serialized_size() + Account::serialized_size() + Amount::serialized_size()
    }

    pub fn serialize(&self, stream: &mut impl Stream) -> Result<()> {
        self.previous.serialize(stream)?;
        self.destination.serialize(stream)?;
        self.balance.serialize(stream)?;
        Ok(())
    }

    pub fn deserialize(stream: &mut impl Stream) -> Result<Self> {
        let mut buffer_32 = [0u8; 32];
        let mut buffer_16 = [0u8; 16];

        stream.read_bytes(&mut buffer_32, 32)?;
        let previous = BlockHash::from_bytes(buffer_32);

        stream.read_bytes(&mut buffer_32, 32)?;
        let destination = Account::from_be_bytes(buffer_32);

        stream.read_bytes(&mut buffer_16, 16)?;
        let balance = Amount::new(u128::from_be_bytes(buffer_16));

        Ok(Self {
            previous,
            destination,
            balance,
        })
    }

    fn clear(&mut self) {
        self.previous = BlockHash::new();
        self.destination = Account::new();
        self.balance = Amount::new(0);
    }
}

#[derive(Clone, Default, Debug)]
pub struct SendBlock {
    pub hashables: SendHashables,
    pub signature: Signature,
    pub work: u64,
    pub hash: RefCell<BlockHash>,
}

impl SendBlock {
    pub fn new(
        previous: &BlockHash,
        destination: &Account,
        balance: &Amount,
        private_key: &RawKey,
        public_key: &PublicKey,
        work: u64,
    ) -> Result<Self> {
        let mut block = Self {
            hashables: SendHashables {
                previous: *previous,
                destination: *destination,
                balance: *balance,
            },
            work,
            signature: Signature::new(),
            hash: RefCell::new(BlockHash::new()),
        };

        let signature = sign_message(private_key, public_key, block.hash().as_bytes())?;
        block.signature = signature;

        Ok(block)
    }

    pub fn read_from_stream(stream: &mut impl Stream) -> Result<Self> {
        let mut result = Self::default();
        result.deserialize(stream)?;
        Ok(result)
    }

    pub fn hash(&self) -> Ref<BlockHash> {
        let mut value = self.hash.borrow();
        if value.is_zero() {
            drop(value);
            let mut x = self.hash.borrow_mut();
            *x = self.generate_hash().unwrap();
            drop(x);
            value = self.hash.borrow();
        }

        value
    }

    pub fn generate_hash(&self) -> Result<BlockHash> {
        let mut blake = RustBlake2b::new();
        blake.init(32)?;
        self.hash_hashables(&mut blake)?;
        let mut result = [0u8; 32];
        blake.finalize(&mut result)?;
        Ok(BlockHash::from_bytes(result))
    }

    pub const fn serialized_size() -> usize {
        SendHashables::serialized_size() + Signature::serialized_size() + std::mem::size_of::<u64>()
    }

    pub fn serialize(&self, stream: &mut impl Stream) -> Result<()> {
        self.hashables.serialize(stream)?;
        self.signature.serialize(stream)?;
        stream.write_bytes(&self.work.to_ne_bytes())
    }

    pub fn deserialize(&mut self, stream: &mut impl Stream) -> Result<()> {
        self.hashables = SendHashables::deserialize(stream)?;
        self.signature = Signature::deserialize(stream)?;

        let mut buffer = [0u8; 8];
        stream.read_bytes(&mut buffer, 8)?;
        self.work = u64::from_ne_bytes(buffer);

        Ok(())
    }

    pub fn zero(&mut self) {
        self.work = 0;
        self.signature = Signature::new();
        self.hashables.clear();
    }

    pub fn set_destination(&mut self, destination: Account) {
        self.hashables.destination = destination;
    }

    pub fn set_previous(&mut self, previous: BlockHash) {
        self.hashables.previous = previous;
    }

    pub fn set_balance(&mut self, balance: Amount) {
        self.hashables.balance = balance;
    }

    pub fn hash_hashables(&self, blake2b: &mut impl Blake2b) -> Result<()> {
        blake2b.update(&self.hashables.previous.to_be_bytes())?;
        blake2b.update(&self.hashables.destination.to_be_bytes())?;
        blake2b.update(&self.hashables.balance.to_be_bytes())?;
        Ok(())
    }

    pub fn valid_predecessor(block_type: BlockType) -> bool {
        match block_type {
            BlockType::Send | BlockType::Receive | BlockType::Open | BlockType::Change => true,
            BlockType::NotABlock | BlockType::State | BlockType::Invalid => false,
        }
    }

    pub fn serialize_json(&self, writer: &mut impl PropertyTreeWriter) -> Result<()> {
        writer.put_string("type", "send")?;
        writer.put_string("previous", &self.hashables.previous.encode_hex())?;
        Ok(())
    }
}

impl PartialEq for SendBlock {
    fn eq(&self, other: &Self) -> bool {
        self.hashables == other.hashables
            && self.signature == other.signature
            && self.work == other.work
    }
}

impl Eq for SendBlock {}

#[cfg(test)]
mod tests {
    use crate::{
        numbers::{validate_message, KeyPair},
        utils::TestStream,
    };

    use super::*;

    // original test: transaction_block.empty
    #[test]
    fn create_send_block() -> Result<()> {
        let key = KeyPair::new();
        let mut block = SendBlock::new(
            &BlockHash::from(0),
            &Account::from(1),
            &Amount::new(13),
            &key.private_key(),
            &key.public_key(),
            2,
        )?;
        let hash = block.hash().to_owned();
        assert!(validate_message(&key.public_key(), hash.as_bytes(), &block.signature).is_ok());

        block.signature.make_invalid();
        assert!(validate_message(&key.public_key(), hash.as_bytes(), &block.signature).is_err());
        Ok(())
    }

    // original test: block.send_serialize
    #[test]
    fn serialize() -> Result<()> {
        let key = KeyPair::new();
        let block1 = SendBlock::new(
            &BlockHash::from(0),
            &Account::from(1),
            &Amount::new(2),
            &key.private_key(),
            &key.public_key(),
            5,
        )?;
        let mut stream = TestStream::new();
        block1.serialize(&mut stream)?;
        assert!(stream.bytes_written() > 0);

        let block2 = SendBlock::read_from_stream(&mut stream)?;
        assert_eq!(block1, block2);
        Ok(())
    }
}
