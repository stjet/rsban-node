use std::cell::Ref;

use crate::{
    numbers::{
        from_string_hex, sign_message, to_string_hex, BlockHash, PublicKey, RawKey, Signature,
    },
    utils::{Blake2b, PropertyTreeReader, PropertyTreeWriter, RustBlake2b, Stream},
};
use anyhow::Result;

use super::{BlockHashFactory, LazyBlockHash};

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
#[derive(Clone, Debug)]
pub struct ReceiveBlock {
    pub work: u64,
    pub signature: Signature,
    pub hashables: ReceiveHashables,
    pub hash: LazyBlockHash,
}

impl ReceiveBlock {
    pub fn new(
        previous: BlockHash,
        source: BlockHash,
        priv_key: &RawKey,
        pub_key: &PublicKey,
        work: u64,
    ) -> Result<Self> {
        let mut result = Self {
            work,
            signature: Signature::new(),
            hashables: ReceiveHashables { previous, source },
            hash: LazyBlockHash::new(),
        };

        let signature = sign_message(priv_key, pub_key, result.hash().as_bytes())?;
        result.signature = signature;

        Ok(result)
    }

    pub fn hash(&self) -> Ref<BlockHash> {
        self.hash.hash(self)
    }

    pub fn hash_hashables(&self, blake2b: &mut impl Blake2b) -> Result<()> {
        blake2b.update(&self.hashables.previous.to_bytes())?;
        blake2b.update(&self.hashables.source.to_bytes())?;
        Ok(())
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

impl BlockHashFactory for ReceiveBlock {
    fn hash(&self) -> BlockHash {
        let mut blake = RustBlake2b::new();
        blake.init(32).unwrap();
        self.hash_hashables(&mut blake).unwrap();
        let mut result = [0u8; 32];
        blake.finalize(&mut result).unwrap();
        BlockHash::from_bytes(result)
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
