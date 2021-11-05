use crate::{
    numbers::{from_string_hex, to_string_hex, BlockHash, Signature},
    utils::{Blake2b, PropertyTreeReader, PropertyTreeWriter},
};
use anyhow::Result;

#[derive(Clone, PartialEq, Eq)]
pub struct ReceiveHashables {
    pub previous: BlockHash,
    pub source: BlockHash,
}

impl ReceiveHashables {
    const fn serialized_size() -> usize {
        BlockHash::serialized_size() + BlockHash::serialized_size()
    }
}
#[derive(Clone)]
pub struct ReceiveBlock {
    pub work: u64,
    pub signature: Signature,
    pub hashables: ReceiveHashables,
}
impl ReceiveBlock {
    pub fn hash(&self, blake2b: &mut impl Blake2b) -> Result<()> {
        blake2b.update(&self.hashables.previous.to_be_bytes())?;
        blake2b.update(&self.hashables.source.to_be_bytes())?;
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
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // original test: block.receive_serialize_json
    #[test]
    fn serialize_json() {}
}
