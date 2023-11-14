use std::fmt;

use rsnano_core::Amount;

/// Information on the status of the inactive cache
#[derive(Clone, Default, PartialEq, Eq)]
pub struct InactiveCacheStatus {
    pub bootstrap_started: bool,
    pub election_started: bool,
    pub confirmed: bool,
    pub tally: Amount,
}

impl fmt::Display for InactiveCacheStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "bootstrap_started={}", self.bootstrap_started)?;
        write!(f, "election_started={}", self.election_started)?;
        write!(f, "confirmed={}", self.confirmed)?;
        write!(f, "tally={}", self.tally.number())
    }
}
