use blake2::digest::{Update, VariableOutput};
use once_cell::sync::Lazy;
use std::{
    cmp::{max, min},
    convert::TryInto,
};

use crate::{
    blocks::{BlockDetails, BlockType},
    epoch::Epoch,
    numbers::{Difficulty, Root},
};

#[derive(Clone, Copy)]
pub enum WorkVersion {
    Unspecified,
    Work1,
}

pub struct WorkThresholds {
    pub epoch_1: u64,
    pub epoch_2: u64,
    pub epoch_2_receive: u64,

    // Automatically calculated. The base threshold is the maximum of all thresholds and is used for all work multiplier calculations
    pub base: u64,

    // Automatically calculated. The entry threshold is the minimum of all thresholds and defines the required work to enter the node, but does not guarantee a block is processed
    pub entry: u64,
}

static PUBLISH_FULL: Lazy<WorkThresholds> = Lazy::new(|| {
    WorkThresholds::new(
        0xffffffc000000000,
        0xfffffff800000000, // 8x higher than epoch_1
        0xfffffe0000000000, // 8x lower than epoch_1
    )
});

static PUBLISH_BETA: Lazy<WorkThresholds> = Lazy::new(|| {
    WorkThresholds::new(
        0xfffff00000000000, // 64x lower than publish_full.epoch_1
        0xfffff00000000000, // same as epoch_1
        0xffffe00000000000, // 2x lower than epoch_1
    )
});

static PUBLISH_DEV: Lazy<WorkThresholds> = Lazy::new(|| {
    WorkThresholds::new(
        0xfe00000000000000, // Very low for tests
        0xffc0000000000000, // 8x higher than epoch_1
        0xf000000000000000, // 8x lower than epoch_1
    )
});

static PUBLISH_TEST: Lazy<WorkThresholds> = Lazy::new(|| {
    WorkThresholds::new(
        get_env_threshold_or_default("NANO_TEST_EPOCH_1", 0xffffffc000000000),
        get_env_threshold_or_default("NANO_TEST_EPOCH_2", 0xfffffff800000000), // 8x higher than epoch_1
        get_env_threshold_or_default("NANO_TEST_EPOCH_2_RECV", 0xfffffe0000000000), // 8x lower than epoch_1
    )
});

fn get_env_threshold_or_default(variable_name: &str, default_value: u64) -> u64 {
    match std::env::var(variable_name) {
        Ok(value) => u64::from_str_radix(&value, 16).expect("could not parse difficulty env var"),
        Err(_) => default_value,
    }
}

impl WorkThresholds {
    pub fn new(epoch_1: u64, epoch_2: u64, epoch_2_receive: u64) -> Self {
        Self {
            epoch_1,
            epoch_2,
            epoch_2_receive,
            base: max(max(epoch_1, epoch_2), epoch_2_receive),
            entry: min(min(epoch_1, epoch_2), epoch_2_receive),
        }
    }

    pub fn publish_full() -> &'static WorkThresholds {
        &PUBLISH_FULL
    }

    pub fn publish_beta() -> &'static WorkThresholds {
        &PUBLISH_BETA
    }

    pub fn publish_dev() -> &'static WorkThresholds {
        &PUBLISH_DEV
    }

    pub fn publish_test() -> &'static WorkThresholds {
        &PUBLISH_TEST
    }

    pub fn threshold_entry(&self, block_type: BlockType, work_version: WorkVersion) -> u64 {
        match block_type {
            BlockType::State => match work_version {
                WorkVersion::Work1 => self.entry,
                _ => {
                    debug_assert!(false, "Invalid version specified to work_threshold_entry");
                    u64::MAX
                }
            },
            _ => self.epoch_1,
        }
    }

    pub fn threshold(&self, details: &BlockDetails) -> u64 {
        match details.epoch {
            Epoch::Epoch2 => {
                if details.is_receive || details.is_epoch {
                    self.epoch_2_receive
                } else {
                    self.epoch_2
                }
            }
            Epoch::Epoch1 | Epoch::Epoch0 => self.epoch_1,
            _ => {
                debug_assert!(
                    false,
                    "Invalid epoch specified to work_v1 ledger work_threshold"
                );
                u64::MAX
            }
        }
    }

    pub fn threshold2(&self, work_version: WorkVersion, details: &BlockDetails) -> u64 {
        match work_version {
            WorkVersion::Work1 => self.threshold(details),
            _ => {
                // Invalid version specified to ledger work_threshold
                debug_assert!(false);
                u64::MAX
            }
        }
    }

    pub fn threshold_base(&self, work_version: WorkVersion) -> u64 {
        match work_version {
            WorkVersion::Work1 => self.base,
            _ => {
                debug_assert!(false, "Invalid version specified to work_threshold_base");
                u64::MAX
            }
        }
    }

    pub fn value(&self, root: &Root, work: u64) -> u64 {
        let mut blake = blake2::VarBlake2b::new_keyed(&[], 8);
        let mut result = 0;
        blake.update(&work.to_ne_bytes());
        blake.update(root.as_bytes());
        blake.finalize_variable(|bytes| {
            result = u64::from_ne_bytes(bytes.try_into().expect("invalid hash length"))
        });
        result
    }

    pub fn normalized_multiplier(&self, multiplier: f64, threshold: u64) -> f64 {
        debug_assert!(multiplier >= 1f64);
        /* Normalization rules
        ratio = multiplier of max work threshold (send epoch 2) from given threshold
        i.e. max = 0xfe00000000000000, given = 0xf000000000000000, ratio = 8.0
        normalized = (multiplier + (ratio - 1)) / ratio;
        Epoch 1
        multiplier	 | normalized
        1.0 		 | 1.0
        9.0 		 | 2.0
        25.0 		 | 4.0
        Epoch 2 (receive / epoch subtypes)
        multiplier	 | normalized
        1.0 		 | 1.0
        65.0 		 | 2.0
        241.0 		 | 4.0
        */
        if threshold == self.epoch_1 || threshold == self.epoch_2_receive {
            let ratio = Difficulty::to_multiplier(self.epoch_2, threshold);
            debug_assert!(ratio >= 1f64);
            let result = (multiplier + (ratio - 1f64)) / ratio;
            debug_assert!(result >= 1f64);
            result
        } else {
            multiplier
        }
    }

    pub fn denormalized_multiplier(&self, multiplier: f64, threshold: u64) -> f64 {
        debug_assert!(multiplier >= 1f64);
        if threshold == self.epoch_1 || threshold == self.epoch_2_receive {
            let ratio = Difficulty::to_multiplier(self.epoch_2, threshold);
            debug_assert!(ratio >= 1f64);
            let result = multiplier * ratio + 1f64 - ratio;
            debug_assert!(result >= 1f64);
            result
        } else {
            multiplier
        }
    }

    pub fn difficulty(&self, work_version: WorkVersion, root: &Root, work: u64) -> u64 {
        match work_version {
            WorkVersion::Work1 => self.value(root, work),
            _ => {
                debug_assert!(false, "Invalid version specified to work_difficulty");
                0
            }
        }
    }

    pub fn validate_entry(&self, work_version: WorkVersion, root: &Root, work: u64) -> bool {
        self.difficulty(work_version, root, work)
            < self.threshold_entry(BlockType::State, work_version)
    }
}