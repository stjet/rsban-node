use num::FromPrimitive;

use crate::{
    config::WorkThresholds,
    ffi::{
        blocks::BlockDto,
        config::{fill_work_thresholds_dto, WorkThresholdsDto},
    },
    secure::LedgerConstants,
};

#[repr(C)]
pub struct LedgerConstantsDto {
    pub work: WorkThresholdsDto,
    pub priv_key: [u8; 32],
    pub pub_key: [u8; 32],
    pub nano_beta_account: [u8; 32],
    pub nano_live_account: [u8; 32],
    pub nano_test_account: [u8; 32],
    pub nano_dev_genesis: BlockDto,
    pub nano_beta_genesis: BlockDto,
    pub nano_live_genesis: BlockDto,
    pub nano_test_genesis: BlockDto,
    pub genesis: BlockDto,
    pub genesis_amount: [u8; 16],
    pub burn_account: [u8; 32],
    pub nano_dev_final_votes_canary_account: [u8; 32],
    pub nano_beta_final_votes_canary_account: [u8; 32],
    pub nano_live_final_votes_canary_account: [u8; 32],
    pub nano_test_final_votes_canary_account: [u8; 32],
    pub final_votes_canary_account: [u8; 32],
    pub nano_dev_final_votes_canary_height: u64,
    pub nano_beta_final_votes_canary_height: u64,
    pub nano_live_final_votes_canary_height: u64,
    pub nano_test_final_votes_canary_height: u64,
    pub final_votes_canary_height: u64,
    pub epoch_1_signer: [u8; 32],
    pub epoch_1_link: [u8; 32],
    pub epoch_2_signer: [u8; 32],
    pub epoch_2_link: [u8; 32],
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ledger_constants_create(
    dto: *mut LedgerConstantsDto,
    work: &WorkThresholdsDto,
    network: u16,
) -> i32 {
    let network = match FromPrimitive::from_u16(network) {
        Some(n) => n,
        None => return -1,
    };

    let work = WorkThresholds::from(work);
    let ledger = LedgerConstants::new(work, network);
    fill_work_thresholds_dto(&mut (*dto).work, &ledger.work);

    //todo fill remaining fields

    0
}
