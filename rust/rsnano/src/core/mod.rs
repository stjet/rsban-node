mod public_key;
pub use public_key::PublicKey;

mod raw_key;
pub use raw_key::RawKey;

mod block_hash;
pub use block_hash::{BlockHash, BlockHashBuilder};

mod signature;
pub use signature::Signature;

mod hash_or_account;
pub use hash_or_account::HashOrAccount;

use std::fmt::Write;

pub(crate) fn encode_hex(i: u128) -> String {
    let mut result = String::with_capacity(32);
    for byte in i.to_ne_bytes() {
        write!(&mut result, "{:02X}", byte).unwrap();
    }
    result
}

pub(crate) fn write_hex_bytes(
    bytes: &[u8],
    f: &mut std::fmt::Formatter,
) -> Result<(), std::fmt::Error> {
    for &byte in bytes {
        write!(f, "{:02X}", byte)?;
    }
    Ok(())
}
