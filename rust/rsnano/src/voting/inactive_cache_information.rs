use rsnano_core::{Account, BlockHash};

use crate::voting::InactiveCacheStatus;
use std::fmt::{Display, Formatter};

/// Information on the status of inactive cache information
#[derive(Default, Clone)]
pub struct InactiveCacheInformation {
    // TODO: change to Instant in the future
    pub arrival: i64,
    pub hash: BlockHash,
    pub status: InactiveCacheStatus,
    pub voters: Vec<(Account, u64)>,
}

impl InactiveCacheInformation {
    pub fn new(
        arrival: i64,
        hash: BlockHash,
        status: InactiveCacheStatus,
        initial_rep: Account,
        initial_timestamp: u64,
    ) -> Self {
        let mut voters = Vec::with_capacity(8);
        voters.push((initial_rep, initial_timestamp));
        Self {
            arrival,
            hash,
            status,
            voters,
        }
    }
}

impl Display for InactiveCacheInformation {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "hash={}", self.hash.to_string())?;
        write!(f, ", arrival={}", self.arrival)?;
        write!(f, ", {}", self.status)?;
        write!(f, ", {} voters", self.voters.len())?;
        for (rep, timestamp) in &self.voters {
            write!(f, " {}/{}", rep, timestamp)?;
        }

        Ok(())
    }
}
