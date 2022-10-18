mod public_key;
pub use public_key::PublicKey;

mod raw_key;
pub use raw_key::RawKey;

use std::fmt::Write;

pub(crate) fn encode_hex(i: u128) -> String {
    let mut result = String::with_capacity(32);
    for byte in i.to_ne_bytes() {
        write!(&mut result, "{:02X}", byte).unwrap();
    }
    result
}
