/**
 * Tag for block signature verification result
 */
#[repr(u8)]
#[derive(PartialEq, Eq, Debug, Clone, Copy, FromPrimitive)]
pub(crate) enum SignatureVerification {
    Unknown = 0,
    Invalid = 1,
    Valid = 2,
    ValidEpoch = 3, // Valid for epoch blocks
}
