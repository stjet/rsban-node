use crate::{
    sign_message, to_hex_string, u64_from_hex_str,
    utils::{PropertyTreeReader, PropertyTreeWriter, Serialize, Stream},
    Account, Amount, BlockHash, BlockHashBuilder, LazyBlockHash, Link, PendingKey, PublicKey,
    RawKey, Root, Signature,
};
use anyhow::Result;

use super::{Block, BlockSideband, BlockType, BlockVisitor};

#[derive(Clone, PartialEq, Eq, Default, Debug)]
pub struct SendHashables {
    pub previous: BlockHash,
    pub destination: Account,
    pub balance: Amount,
}

impl SendHashables {
    pub fn serialized_size() -> usize {
        BlockHash::serialized_size() + Account::serialized_size() + Amount::serialized_size()
    }

    pub fn serialize(&self, stream: &mut dyn Stream) -> Result<()> {
        self.previous.serialize(stream)?;
        self.destination.serialize(stream)?;
        self.balance.serialize(stream)?;
        Ok(())
    }

    pub fn deserialize(stream: &mut dyn Stream) -> Result<Self> {
        let mut buffer_32 = [0u8; 32];
        let mut buffer_16 = [0u8; 16];

        stream.read_bytes(&mut buffer_32, 32)?;
        let previous = BlockHash::from_bytes(buffer_32);

        stream.read_bytes(&mut buffer_32, 32)?;
        let destination = Account::from_bytes(buffer_32);

        stream.read_bytes(&mut buffer_16, 16)?;
        let balance = Amount::new(u128::from_be_bytes(buffer_16));

        Ok(Self {
            previous,
            destination,
            balance,
        })
    }

    fn clear(&mut self) {
        self.previous = BlockHash::zero();
        self.destination = Account::zero();
        self.balance = Amount::new(0);
    }
}

impl From<&SendHashables> for BlockHash {
    fn from(hashables: &SendHashables) -> Self {
        BlockHashBuilder::new()
            .update(hashables.previous.as_bytes())
            .update(hashables.destination.as_bytes())
            .update(&hashables.balance.to_be_bytes())
            .build()
    }
}

#[derive(Clone, Default, Debug)]
pub struct SendBlock {
    pub hashables: SendHashables,
    pub signature: Signature,
    pub work: u64,
    pub hash: LazyBlockHash,
    pub sideband: Option<BlockSideband>,
}

impl SendBlock {
    pub fn new(
        previous: &BlockHash,
        destination: &Account,
        balance: &Amount,
        private_key: &RawKey,
        public_key: &PublicKey,
        work: u64,
    ) -> Self {
        let hashables = SendHashables {
            previous: *previous,
            destination: *destination,
            balance: *balance,
        };

        let hash = LazyBlockHash::new();
        let signature = sign_message(private_key, public_key, hash.hash(&hashables).as_bytes());

        Self {
            hashables,
            work,
            signature,
            hash,
            sideband: None,
        }
    }

    pub fn deserialize(stream: &mut dyn Stream) -> Result<Self> {
        let hashables = SendHashables::deserialize(stream)?;
        let signature = Signature::deserialize(stream)?;

        let mut buffer = [0u8; 8];
        stream.read_bytes(&mut buffer, 8)?;
        let work = u64::from_be_bytes(buffer);
        Ok(SendBlock {
            hashables,
            signature,
            work,
            hash: LazyBlockHash::new(),
            sideband: None,
        })
    }

    pub fn serialized_size() -> usize {
        SendHashables::serialized_size() + Signature::serialized_size() + std::mem::size_of::<u64>()
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

    pub fn deserialize_json(reader: &impl PropertyTreeReader) -> Result<Self> {
        let previous = BlockHash::decode_hex(reader.get_string("previous")?)?;
        let destination = Account::decode_account(reader.get_string("destination")?)?;
        let balance = Amount::decode_dec(reader.get_string("balance")?)?;
        let signature = Signature::decode_hex(reader.get_string("signature")?)?;
        let work = u64_from_hex_str(reader.get_string("work")?)?;
        Ok(SendBlock {
            hashables: SendHashables {
                previous,
                destination,
                balance,
            },
            signature,
            work,
            hash: LazyBlockHash::new(),
            sideband: None,
        })
    }

    pub fn pending_key(&self) -> PendingKey {
        PendingKey::new(self.hashables.destination, self.hash())
    }

    pub fn mandatory_destination(&self) -> &Account {
        &self.hashables.destination
    }
}

pub fn valid_send_block_predecessor(block_type: BlockType) -> bool {
    match block_type {
        BlockType::LegacySend
        | BlockType::LegacyReceive
        | BlockType::LegacyOpen
        | BlockType::LegacyChange => true,
        BlockType::NotABlock | BlockType::State | BlockType::Invalid => false,
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

impl Block for SendBlock {
    fn sideband(&'_ self) -> Option<&'_ BlockSideband> {
        self.sideband.as_ref()
    }

    fn set_sideband(&mut self, sideband: BlockSideband) {
        self.sideband = Some(sideband);
    }

    fn block_type(&self) -> BlockType {
        BlockType::LegacySend
    }

    fn account(&self) -> Account {
        Account::zero()
    }

    fn hash(&self) -> BlockHash {
        self.hash.hash(&self.hashables)
    }

    fn link(&self) -> Link {
        Link::zero()
    }

    fn block_signature(&self) -> &Signature {
        &self.signature
    }

    fn set_block_signature(&mut self, signature: &Signature) {
        self.signature = signature.clone();
    }

    fn set_work(&mut self, work: u64) {
        self.work = work;
    }

    fn work(&self) -> u64 {
        self.work
    }

    fn previous(&self) -> BlockHash {
        self.hashables.previous
    }

    fn serialize(&self, stream: &mut dyn Stream) -> Result<()> {
        self.hashables.serialize(stream)?;
        self.signature.serialize(stream)?;
        stream.write_bytes(&self.work.to_be_bytes())
    }

    fn serialize_json(&self, writer: &mut dyn PropertyTreeWriter) -> Result<()> {
        writer.put_string("type", "send")?;
        writer.put_string("previous", &self.hashables.previous.encode_hex())?;
        writer.put_string("destination", &self.hashables.destination.encode_account())?;
        writer.put_string("balance", &self.hashables.balance.to_string_dec())?;
        writer.put_string("work", &to_hex_string(self.work))?;
        writer.put_string("signature", &self.signature.encode_hex())?;
        Ok(())
    }

    fn root(&self) -> Root {
        self.previous().into()
    }

    fn visit(&self, visitor: &mut dyn BlockVisitor) {
        visitor.send_block(self);
    }

    fn balance(&self) -> Amount {
        self.hashables.balance
    }

    fn source(&self) -> Option<BlockHash> {
        None
    }

    fn representative(&self) -> Option<Account> {
        None
    }

    fn visit_mut(&mut self, visitor: &mut dyn super::MutableBlockVisitor) {
        visitor.send_block(self);
    }

    fn valid_predecessor(&self, block_type: BlockType) -> bool {
        valid_send_block_predecessor(block_type)
    }

    fn destination(&self) -> Option<Account> {
        Some(self.hashables.destination)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        utils::{MemoryStream, TestPropertyTree},
        validate_message, KeyPair,
    };

    #[test]
    fn create_send_block() {
        let key = KeyPair::new();
        let mut block = SendBlock::new(
            &BlockHash::from(0),
            &Account::from(1),
            &Amount::new(13),
            &key.private_key(),
            &key.public_key(),
            2,
        );

        assert_eq!(block.root(), block.previous().into());
        let hash = block.hash().to_owned();
        assert!(validate_message(&key.public_key(), hash.as_bytes(), &block.signature).is_ok());

        block.signature.make_invalid();
        assert!(validate_message(&key.public_key(), hash.as_bytes(), &block.signature).is_err());
    }

    // original test: block.send_serialize
    // original test: send_block.deserialize
    #[test]
    fn serialize() {
        let key = KeyPair::new();
        let block1 = SendBlock::new(
            &BlockHash::from(0),
            &Account::from(1),
            &Amount::new(2),
            &key.private_key(),
            &key.public_key(),
            5,
        );
        let mut stream = MemoryStream::new();
        block1.serialize(&mut stream).unwrap();
        assert_eq!(SendBlock::serialized_size(), stream.bytes_written());

        let block2 = SendBlock::deserialize(&mut stream).unwrap();
        assert_eq!(block1, block2);
    }

    // originial test: block.send_serialize_json
    #[test]
    fn serialize_json() {
        let key = KeyPair::new();
        let block1 = SendBlock::new(
            &BlockHash::from(0),
            &Account::from(1),
            &Amount::new(2),
            &key.private_key(),
            &key.public_key(),
            5,
        );

        let mut ptree = TestPropertyTree::new();
        block1.serialize_json(&mut ptree).unwrap();

        let block2 = SendBlock::deserialize_json(&ptree).unwrap();
        assert_eq!(block1, block2);
    }
}
