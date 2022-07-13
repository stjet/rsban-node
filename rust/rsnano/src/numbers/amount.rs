use crate::utils::Stream;
use anyhow::Result;

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct Amount {
    value: u128, // native endian!
}

impl Amount {
    pub fn new(value: u128) -> Self {
        Self { value }
    }

    pub fn zero() -> Self {
        Self::new(0)
    }

    pub fn from_be_bytes(bytes: [u8; 16]) -> Self {
        Self {
            value: u128::from_be_bytes(bytes),
        }
    }

    pub fn from_le_bytes(bytes: [u8; 16]) -> Self {
        Self {
            value: u128::from_le_bytes(bytes),
        }
    }

    pub const fn serialized_size() -> usize {
        std::mem::size_of::<u128>()
    }

    pub fn serialize(&self, stream: &mut dyn Stream) -> Result<()> {
        stream.write_bytes(&self.value.to_be_bytes())
    }

    pub fn deserialize(stream: &mut dyn Stream) -> Result<Self> {
        let mut buffer = [0u8; 16];
        let len = buffer.len();
        stream.read_bytes(&mut buffer, len)?;
        Ok(Amount::new(u128::from_be_bytes(buffer)))
    }

    pub fn to_be_bytes(self) -> [u8; 16] {
        self.value.to_be_bytes()
    }

    pub fn to_le_bytes(self) -> [u8; 16] {
        self.value.to_le_bytes()
    }

    pub fn encode_hex(&self) -> String {
        format!("{:032X}", self.value)
    }

    pub fn decode_hex(s: impl AsRef<str>) -> Result<Self> {
        let value = u128::from_str_radix(s.as_ref(), 16)?;
        Ok(Amount::new(value))
    }

    pub fn decode_dec(s: impl AsRef<str>) -> Result<Self> {
        Ok(Self::new(s.as_ref().parse::<u128>()?))
    }

    pub fn to_string_dec(self) -> String {
        self.value.to_string()
    }

    pub fn format_balance(&self, scale: u128, precision: i32, group_digits: bool) -> String {
        "".to_string() //todo
    }
}

impl From<u128> for Amount {
    fn from(value: u128) -> Self {
        Amount::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{KXRB_RATIO, MXRB_RATIO, XRB_RATIO};

    #[test]
    #[ignore = "todo"]
    fn format_balance() {
        assert_eq!("0", Amount::new(0).format_balance(*MXRB_RATIO, 0, false));
        assert_eq!("0", Amount::new(0).format_balance(*MXRB_RATIO, 2, true));
        assert_eq!(
            "340,282,366",
            Amount::decode_hex("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")
                .unwrap()
                .format_balance(*MXRB_RATIO, 0, true)
        );
        assert_eq!(
            "340,282,366.920938463463374607431768211455",
            Amount::decode_hex("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")
                .unwrap()
                .format_balance(*MXRB_RATIO, 64, true)
        );
        assert_eq!(
            "340,282,366,920,938,463,463,374,607,431,768,211,455",
            Amount::decode_hex("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")
                .unwrap()
                .format_balance(1, 4, true)
        );
        assert_eq!(
            "340,282,366",
            Amount::decode_hex("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE")
                .unwrap()
                .format_balance(*MXRB_RATIO, 0, true)
        );
        assert_eq!(
            "340,282,366.920938463463374607431768211454",
            Amount::decode_hex("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE")
                .unwrap()
                .format_balance(*MXRB_RATIO, 64, true)
        );
        assert_eq!(
            "340282366920938463463374607431768211454",
            Amount::decode_hex("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE")
                .unwrap()
                .format_balance(1, 4, false)
        );
        assert_eq!(
            "170,141,183",
            Amount::decode_hex("7FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE")
                .unwrap()
                .format_balance(*MXRB_RATIO, 0, true)
        );
        assert_eq!(
            "170,141,183.460469231731687303715884105726",
            Amount::decode_hex("7FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE")
                .unwrap()
                .format_balance(*MXRB_RATIO, 64, true)
        );
        assert_eq!(
            "170141183460469231731687303715884105726",
            Amount::decode_hex("7FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE")
                .unwrap()
                .format_balance(1, 4, false)
        );
        assert_eq!(
            "1",
            Amount::decode_dec("1000000000000000000000000000000")
                .unwrap()
                .format_balance(*MXRB_RATIO, 2, true)
        );
        assert_eq!(
            "1.2",
            Amount::decode_dec("1200000000000000000000000000000")
                .unwrap()
                .format_balance(*MXRB_RATIO, 2, true)
        );
        assert_eq!(
            "1.23",
            Amount::decode_dec("1230000000000000000000000000000")
                .unwrap()
                .format_balance(*MXRB_RATIO, 2, true)
        );
        assert_eq!(
            "1.2",
            Amount::decode_dec("1230000000000000000000000000000")
                .unwrap()
                .format_balance(*MXRB_RATIO, 1, true)
        );
        assert_eq!(
            "1",
            Amount::decode_dec("1230000000000000000000000000000")
                .unwrap()
                .format_balance(*MXRB_RATIO, 0, true)
        );
        assert_eq!(
            "< 0.01",
            Amount::new(*XRB_RATIO * 10).format_balance(*MXRB_RATIO, 2, true)
        );
        assert_eq!(
            "< 0.1",
            Amount::new(*XRB_RATIO * 10).format_balance(*MXRB_RATIO, 1, true)
        );
        assert_eq!(
            "< 1",
            Amount::new(*XRB_RATIO * 10).format_balance(*MXRB_RATIO, 0, true)
        );
        assert_eq!(
            "< 0.01",
            Amount::new(*XRB_RATIO * 9999).format_balance(*MXRB_RATIO, 2, true)
        );
        assert_eq!(
            "0.01",
            Amount::new(*XRB_RATIO * 10000).format_balance(*MXRB_RATIO, 2, true)
        );
        assert_eq!(
            "123456789",
            Amount::new(*MXRB_RATIO * 123456789).format_balance(*MXRB_RATIO, 2, false)
        );
        assert_eq!(
            "123,456,789",
            Amount::new(*MXRB_RATIO * 123456789).format_balance(*MXRB_RATIO, 2, true)
        );
        assert_eq!(
            "123,456,789.12",
            Amount::new(*MXRB_RATIO * 123456789 + *KXRB_RATIO * 123).format_balance(
                *MXRB_RATIO,
                2,
                true
            )
        );
        //assert_eq! ("12-3456-789+123", Amount::new (*MXRB_RATIO * 123456789 + *KXRB_RATIO * 123).format_balance (*MXRB_RATIO, 4, true, std::locale (std::cout.getloc (), new test_punct)));
    }
}
