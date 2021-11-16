use crate::numbers::{Link, PublicKey};
use std::collections::HashMap;

/**
 * Tag for which epoch an entry belongs to
 */

#[repr(u8)]
#[derive(PartialEq, Eq, Debug, Clone, Copy, FromPrimitive, Hash)]
pub enum Epoch {
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

struct EpochInfo {
    pub signer: PublicKey,
    pub link: Link,
}

pub struct Epochs {
    epochs: HashMap<Epoch, EpochInfo>,
}

impl Epochs {
    pub fn new() -> Self {
        Self {
            epochs: HashMap::new(),
        }
    }

    pub fn add(&mut self, epoch: Epoch, signer: PublicKey, link: Link) {
        self.epochs.insert(epoch, EpochInfo { signer, link });
    }

    pub fn is_epoch_link(&self, link: &Link) -> bool {
        self.epochs.values().any(|x| &x.link == link)
    }

    pub fn link(&self, epoch: Epoch) -> Option<&Link> {
        self.epochs.get(&epoch).map(|x| &x.link)
    }

    pub fn signer(&self, epoch: Epoch) -> Option<&PublicKey> {
        self.epochs.get(&epoch).map(|x| &x.signer)
    }

    pub fn epoch(&self, link: &Link) -> Option<Epoch> {
        for (k, v) in &self.epochs {
            if &v.link == link {
                return Some(*k);
            }
        }

        None
    }

    /** Checks that new_epoch is 1 version higher than epoch */
    pub fn is_sequential(epoch: Epoch, new_epoch: Epoch) -> bool {
        let epoch_id = epoch as u8;
        let new_epoch_id = new_epoch as u8;
        epoch_id >= Epoch::Epoch0 as u8 && new_epoch_id == epoch_id + 1
    }
}
