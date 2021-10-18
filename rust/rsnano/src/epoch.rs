/**
 * Tag for which epoch an entry belongs to
 */

#[repr(u8)]
#[derive(PartialEq, Eq, Debug, Clone, Copy, FromPrimitive)]
pub(crate) enum Epoch {
    Invalid = 0,
    Unspecified = 1,
    Epoch0 = 2,
    Epoch1 = 3,
    Epoch2 = 4,
}

impl Epoch {
    pub const EPOCH_BEGIN: Epoch = Epoch::Epoch0;
    pub const MAX: Epoch = Epoch::Epoch2;
}
