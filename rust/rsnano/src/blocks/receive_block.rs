use crate::{
    from_string_hex, sign_message, to_string_hex, Account, Block, BlockHash, BlockHashBuilder,
    BlockSideband, BlockType, LazyBlockHash, Link, PropertyTreeReader, PropertyTreeWriter,
    PublicKey, RawKey, Signature, Stream,
};
use anyhow::Result;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ReceiveHashables {
    pub previous: BlockHash,
    pub source: BlockHash,
}

impl ReceiveHashables {
    const fn serialized_size() -> usize {
        BlockHash::serialized_size() + BlockHash::serialized_size()
    }
}

impl From<&ReceiveHashables> for BlockHash {
    fn from(hashables: &ReceiveHashables) -> Self {
        BlockHashBuilder::new()
            .update(hashables.previous.as_bytes())
            .update(hashables.source.as_bytes())
            .build()
    }
}

#[derive(Clone, Debug)]
pub struct ReceiveBlock {
    pub work: u64,
    pub signature: Signature,
    pub hashables: ReceiveHashables,
    pub hash: LazyBlockHash,
    pub sideband: Option<BlockSideband>,
}

impl ReceiveBlock {
    pub fn new(
        previous: BlockHash,
        source: BlockHash,
        priv_key: &RawKey,
        pub_key: &PublicKey,
        work: u64,
    ) -> Result<Self> {
        let hashables = ReceiveHashables { previous, source };
        let hash = LazyBlockHash::new();
        let signature = sign_message(priv_key, pub_key, hash.hash(&hashables).as_bytes())?;

        Ok(Self {
            work,
            signature,
            hashables,
            hash,
            sideband: None,
        })
    }

    pub const fn serialized_size() -> usize {
        ReceiveHashables::serialized_size()
            + Signature::serialized_size()
            + std::mem::size_of::<u64>()
    }

    pub fn serialize_json(&self, writer: &mut impl PropertyTreeWriter) -> Result<()> {
        writer.put_string("type", "receive")?;
        writer.put_string("previous", &self.hashables.previous.encode_hex())?;
        writer.put_string("source", &self.hashables.source.encode_hex())?;
        writer.put_string("work", &to_string_hex(self.work))?;
        writer.put_string("signature", &self.signature.encode_hex())?;
        Ok(())
    }

    pub fn deserialize_json(reader: &impl PropertyTreeReader) -> Result<Self> {
        let previous = BlockHash::decode_hex(reader.get_string("previous")?)?;
        let source = BlockHash::decode_hex(reader.get_string("source")?)?;
        let signature = Signature::decode_hex(reader.get_string("signature")?)?;
        let work = from_string_hex(reader.get_string("work")?)?;
        Ok(Self {
            work,
            signature,
            hashables: ReceiveHashables { previous, source },
            hash: LazyBlockHash::new(),
            sideband: None,
        })
    }

    pub fn deserialize(stream: &mut impl Stream) -> Result<Self> {
        let previous = BlockHash::deserialize(stream)?;
        let source = BlockHash::deserialize(stream)?;
        let signature = Signature::deserialize(stream)?;
        let mut work_bytes = [0u8; 8];
        stream.read_bytes(&mut work_bytes, 8)?;
        let work = u64::from_be_bytes(work_bytes);
        Ok(Self {
            work,
            signature,
            hashables: ReceiveHashables { previous, source },
            hash: LazyBlockHash::new(),
            sideband: None,
        })
    }

    pub fn serialize(&self, stream: &mut impl Stream) -> Result<()> {
        self.hashables.previous.serialize(stream)?;
        self.hashables.source.serialize(stream)?;
        self.signature.serialize(stream)?;
        stream.write_bytes(&self.work.to_be_bytes())?;
        Ok(())
    }
}

impl PartialEq for ReceiveBlock {
    fn eq(&self, other: &Self) -> bool {
        self.work == other.work
            && self.signature == other.signature
            && self.hashables == other.hashables
    }
}

impl Eq for ReceiveBlock {}

impl Block for ReceiveBlock {
    fn sideband(&'_ self) -> Option<&'_ BlockSideband> {
        self.sideband.as_ref()
    }

    fn set_sideband(&mut self, sideband: BlockSideband) {
        self.sideband = Some(sideband)
    }

    fn block_type(&self) -> BlockType {
        BlockType::Receive
    }

    fn account(&self) -> &crate::numbers::Account {
        Account::zero()
    }

    fn hash(&self) -> BlockHash {
        self.hash.hash(&self.hashables)
    }

    fn link(&self) -> crate::Link {
        Link::new()
    }

    fn block_signature(&self) -> &Signature {
        &self.signature
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        numbers::KeyPair,
        utils::{TestPropertyTree, TestStream},
    };

    use super::*;

    // original test: block.receive_serialize
    // original test: receive_block.deserialize
    #[test]
    fn serialize() -> Result<()> {
        let key1 = KeyPair::new();
        let block1 = ReceiveBlock::new(
            BlockHash::from(0),
            BlockHash::from(1),
            &key1.private_key(),
            &key1.public_key(),
            4,
        )?;
        let mut stream = TestStream::new();
        block1.serialize(&mut stream)?;
        assert_eq!(ReceiveBlock::serialized_size(), stream.bytes_written());

        let block2 = ReceiveBlock::deserialize(&mut stream)?;
        assert_eq!(block1, block2);
        Ok(())
    }

    // original test: block.receive_serialize_json
    #[test]
    fn serialize_json() -> Result<()> {
        let key1 = KeyPair::new();
        let block1 = ReceiveBlock::new(
            BlockHash::from(0),
            BlockHash::from(1),
            &key1.private_key(),
            &key1.public_key(),
            4,
        )?;
        let mut ptree = TestPropertyTree::new();
        block1.serialize_json(&mut ptree)?;

        let block2 = ReceiveBlock::deserialize_json(&ptree)?;
        assert_eq!(block1, block2);
        Ok(())
    }
}
