use crate::{
    core::BlockHandle,
    work::{fill_work_thresholds_dto, WorkThresholdsDto},
};
use num::FromPrimitive;
use rsnano_core::{
    work::WorkThresholds, Account, Amount, BlockEnum, Epoch, Epochs, KeyPair, Link, PublicKey,
};
use rsnano_ledger::LedgerConstants;
use std::{convert::TryFrom, ops::Deref, sync::Arc};

#[repr(C)]
pub struct LedgerConstantsDto {
    pub work: WorkThresholdsDto,
    pub priv_key: [u8; 32],
    pub pub_key: [u8; 32],
    pub nano_beta_account: [u8; 32],
    pub nano_live_account: [u8; 32],
    pub nano_test_account: [u8; 32],
    pub nano_dev_genesis: *mut BlockHandle,
    pub nano_beta_genesis: *mut BlockHandle,
    pub nano_live_genesis: *mut BlockHandle,
    pub nano_test_genesis: *mut BlockHandle,
    pub genesis: *mut BlockHandle,
    pub genesis_amount: [u8; 16],
    pub burn_account: [u8; 32],
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

    fill_ledger_constants_dto(&mut (*dto), &ledger);
    0
}

fn block_to_block_handle(block: &Arc<BlockEnum>) -> *mut BlockHandle {
    Box::into_raw(Box::new(BlockHandle(Arc::clone(block))))
}

pub fn fill_ledger_constants_dto(dto: &mut LedgerConstantsDto, ledger: &LedgerConstants) {
    fill_work_thresholds_dto(&mut dto.work, &ledger.work);
    dto.pub_key = *ledger.zero_key.public_key().as_bytes();
    dto.priv_key = *ledger.zero_key.private_key().as_bytes();
    dto.nano_beta_account = *ledger.nano_beta_account.as_bytes();
    dto.nano_live_account = *ledger.nano_live_account.as_bytes();
    dto.nano_test_account = *ledger.nano_test_account.as_bytes();
    dto.nano_dev_genesis = block_to_block_handle(&ledger.nano_dev_genesis);
    dto.nano_beta_genesis = block_to_block_handle(&ledger.nano_beta_genesis);
    dto.nano_live_genesis = block_to_block_handle(&ledger.nano_live_genesis);
    dto.nano_test_genesis = block_to_block_handle(&ledger.nano_test_genesis);
    dto.genesis = block_to_block_handle(&ledger.genesis);
    dto.genesis_amount = ledger.genesis_amount.to_be_bytes();
    dto.burn_account = *ledger.burn_account.as_bytes();
    dto.epoch_1_signer = *ledger.epochs.signer(Epoch::Epoch1).unwrap().as_bytes();
    dto.epoch_1_link = *ledger.epochs.link(Epoch::Epoch1).unwrap().as_bytes();
    dto.epoch_2_signer = *ledger.epochs.signer(Epoch::Epoch2).unwrap().as_bytes();
    dto.epoch_2_link = *ledger.epochs.link(Epoch::Epoch2).unwrap().as_bytes();
}

impl TryFrom<&LedgerConstantsDto> for LedgerConstants {
    type Error = anyhow::Error;

    fn try_from(value: &LedgerConstantsDto) -> Result<Self, Self::Error> {
        let mut epochs = Epochs::new();
        epochs.add(
            Epoch::Epoch1,
            PublicKey::from_bytes(value.epoch_1_signer),
            Link::from_bytes(value.epoch_1_link),
        );
        epochs.add(
            Epoch::Epoch2,
            PublicKey::from_bytes(value.epoch_2_signer),
            Link::from_bytes(value.epoch_2_link),
        );

        let genesis = unsafe { &*value.genesis }.deref().clone();
        let genesis_account = genesis.account_field().unwrap();
        let ledger = LedgerConstants {
            work: (&value.work).into(),
            zero_key: KeyPair::from_priv_key_bytes(&value.priv_key)?,
            nano_beta_account: Account::from_bytes(value.nano_beta_account),
            nano_live_account: Account::from_bytes(value.nano_live_account),
            nano_test_account: Account::from_bytes(value.nano_test_account),
            nano_dev_genesis: unsafe { &*value.nano_dev_genesis }.deref().clone(),
            nano_beta_genesis: unsafe { &*value.nano_beta_genesis }.deref().clone(),
            nano_live_genesis: unsafe { &*value.nano_live_genesis }.deref().clone(),
            nano_test_genesis: unsafe { &*value.nano_test_genesis }.deref().clone(),
            genesis_account,
            genesis,
            genesis_amount: Amount::from_be_bytes(value.genesis_amount),
            burn_account: Account::from_bytes(value.burn_account),
            epochs,
        };

        // We have to free the memory for the block handles!
        unsafe {
            drop(Box::from_raw(value.nano_dev_genesis));
            drop(Box::from_raw(value.nano_beta_genesis));
            drop(Box::from_raw(value.nano_live_genesis));
            drop(Box::from_raw(value.nano_test_genesis));
            drop(Box::from_raw(value.genesis));
        }

        Ok(ledger)
    }
}
