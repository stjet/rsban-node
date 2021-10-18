use num::FromPrimitive;

use crate::epoch::Epoch;

// Epoch is bit packed in BlockDetails. That's why it's max is limited to 4 bits
const_assert!((Epoch::MAX as u8) < (1 << 5));

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct BlockDetails {
    pub epoch: Epoch,
    pub is_send: bool,
    pub is_receive: bool,
    pub is_epoch: bool,
}

impl BlockDetails {
    pub fn new(epoch: Epoch, is_send: bool, is_receive: bool, is_epoch: bool) -> Self {
        Self {
            epoch,
            is_send,
            is_receive,
            is_epoch,
        }
    }
    pub fn packed(&self) -> u8 {
        let mut result = self.epoch as u8;
        if self.is_send {
            result |= 0b1000_0000;
        }
        if self.is_receive {
            result |= 0b0100_0000;
        }
        if self.is_epoch {
            result |= 0b0010_0000;
        }

        result
    }

    pub fn unpack(value: u8) -> Option<Self> {
        let epoch_mask = 0b0001_1111u8;
        let epoch = match FromPrimitive::from_u8(value & epoch_mask) {
            Some(e) => e,
            None => return None,
        };

        Some(BlockDetails {
            epoch,
            is_send: (0b1000_0000 & value) != 0,
            is_receive: (0b0100_0000 & value) != 0,
            is_epoch: (0b0010_0000 & value) != 0,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_block_details() {
        let details_send = BlockDetails::new(Epoch::Epoch0, true, false, false);
        assert_eq!(details_send.is_send, true);
        assert_eq!(details_send.is_receive, false);
        assert_eq!(details_send.is_epoch, false);
        assert_eq!(details_send.epoch, Epoch::Epoch0);

        let details_receive = BlockDetails::new(Epoch::Epoch1, false, true, false);
        assert_eq!(details_receive.is_send, false);
        assert_eq!(details_receive.is_receive, true);
        assert_eq!(details_receive.is_epoch, false);
        assert_eq!(details_receive.epoch, Epoch::Epoch1);

        let details_epoch = BlockDetails::new(Epoch::Epoch2, false, false, true);
        assert_eq!(details_epoch.is_send, false);
        assert_eq!(details_epoch.is_receive, false);
        assert_eq!(details_epoch.is_epoch, true);
        assert_eq!(details_epoch.epoch, Epoch::Epoch2);

        let details_none = BlockDetails::new(Epoch::Unspecified, false, false, false);
        assert_eq!(details_none.is_send, false);
        assert_eq!(details_none.is_receive, false);
        assert_eq!(details_none.is_epoch, false);
        assert_eq!(details_none.epoch, Epoch::Unspecified);
    }

    #[test]
    fn test_pack_and_unpack() {
        let details_send = BlockDetails::new(Epoch::Epoch0, true, false, false);
        assert_eq!(details_send.packed(), 0b1000_0010);
        assert_eq!(
            BlockDetails::unpack(details_send.packed()).unwrap(),
            details_send
        );

        let details_receive = BlockDetails::new(Epoch::Epoch1, false, true, false);
        assert_eq!(details_receive.packed(), 0b0100_0011);
        assert_eq!(
            BlockDetails::unpack(details_receive.packed()).unwrap(),
            details_receive
        );

        let details_epoch = BlockDetails::new(Epoch::Epoch2, false, false, true);
        assert_eq!(details_epoch.packed(), 0b0010_0100);
        assert_eq!(
            BlockDetails::unpack(details_epoch.packed()).unwrap(),
            details_epoch
        );

        let details_none = BlockDetails::new(Epoch::Unspecified, false, false, false);
        assert_eq!(details_none.packed(), 0b0000_0001);
        assert_eq!(
            BlockDetails::unpack(details_none.packed()).unwrap(),
            details_none
        );
    }
}
