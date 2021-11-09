use std::cmp::{max, min};

use once_cell::sync::Lazy;

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
}
