use crate::{
    numbers::{from_string_hex, to_string_hex, Account, BlockHash, Signature},
    utils::{Blake2b, PropertyTreeReader, PropertyTreeWriter, Stream},
};
use anyhow::Result;

#[derive(Clone, PartialEq, Eq)]
pub struct ChangeHashables {
    pub previous: BlockHash,
    pub representative: Account,
}

impl ChangeHashables {
    const fn serialized_size() -> usize {
        BlockHash::serialized_size() + Account::serialized_size()
    }
}

#[derive(Clone)]
pub struct ChangeBlock {
    pub work: u64,
    pub signature: Signature,
    pub hashables: ChangeHashables,
}

impl ChangeBlock {
    pub const fn serialized_size() -> usize {
        ChangeHashables::serialized_size()
            + Signature::serialized_size()
            + std::mem::size_of::<u64>()
    }

    pub fn hash(&self, blake2b: &mut impl Blake2b) -> Result<()> {
        blake2b.update(&self.hashables.previous.to_be_bytes())?;
        blake2b.update(&self.hashables.representative.to_bytes())?;
        Ok(())
    }

    pub fn serialize(&self, stream: &mut impl Stream) -> Result<()> {
        self.hashables.previous.serialize(stream)?;
        self.hashables.representative.serialize(stream)?;
        self.signature.serialize(stream)?;
        stream.write_bytes(&self.work.to_be_bytes())?;
        Ok(())
    }

    pub fn deserialize(&mut self, stream: &mut impl Stream) -> Result<()> {
        self.hashables.previous = BlockHash::deserialize(stream)?;
        self.hashables.representative.deserialize(stream)?;
        self.signature = Signature::deserialize(stream)?;
        let mut work_bytes = [0u8; 8];
        stream.read_bytes(&mut work_bytes, 8)?;
        self.work = u64::from_be_bytes(work_bytes);
        Ok(())
    }

    pub fn serialize_json(&self, writer: &mut impl PropertyTreeWriter) -> Result<()> {
        writer.put_string("type", "change")?;
        writer.put_string("previous", &self.hashables.previous.encode_hex())?;
        writer.put_string(
            "representative",
            &self.hashables.representative.encode_account(),
        )?;
        writer.put_string("work", &to_string_hex(self.work))?;
        writer.put_string("signature", &self.signature.encode_hex())?;
        Ok(())
    }

    pub fn deserialize_json(reader: &impl PropertyTreeReader) -> Result<Self> {
        let previous = BlockHash::decode_hex(reader.get_string("previous")?)?;
        let representative = Account::decode_account(reader.get_string("representative")?)?;
        let work = from_string_hex(reader.get_string("work")?)?;
        let signature = Signature::decode_hex(reader.get_string("signature")?)?;
        Ok(Self {
            work,
            signature,
            hashables: ChangeHashables {
                previous,
                representative,
            },
        })
    }
}
