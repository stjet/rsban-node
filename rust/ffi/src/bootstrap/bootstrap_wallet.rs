use super::{
    bootstrap_attempt::BootstrapAttemptHandle, bootstrap_connections::BootstrapConnectionsHandle,
    BootstrapInitiatorHandle,
};
use crate::{
    block_processing::BlockProcessorHandle, ledger::datastore::LedgerHandle,
    utils::ThreadPoolHandle, websocket::WebsocketListenerHandle, NodeConfigDto, StatHandle,
};
use rsnano_node::{
    bootstrap::{BootstrapAttemptWallet, BootstrapAttemptWalletExt, BootstrapStrategy},
    config::NodeConfig,
};
use std::{
    ffi::{c_char, CStr},
    ops::Deref,
    sync::Arc,
};

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_wallet_create(
    websocket_server: *mut WebsocketListenerHandle,
    block_processor: &BlockProcessorHandle,
    bootstrap_initiator: *const BootstrapInitiatorHandle,
    ledger: *const LedgerHandle,
    id: *const c_char,
    incremental_id: u64,
    connections: &BootstrapConnectionsHandle,
    workers: &ThreadPoolHandle,
    config: &NodeConfigDto,
    stats: &StatHandle,
) -> *mut BootstrapAttemptHandle {
    let id_str = CStr::from_ptr(id).to_str().unwrap();
    let websocket_server = if websocket_server.is_null() {
        None
    } else {
        Some(Arc::clone((*websocket_server).deref()))
    };
    let bootstrap_initiator = Arc::clone(&*bootstrap_initiator);
    let ledger = Arc::clone(&*ledger);
    let config = NodeConfig::try_from(config).unwrap();
    BootstrapAttemptHandle::new(Arc::new(BootstrapStrategy::Wallet(Arc::new(
        BootstrapAttemptWallet::new(
            websocket_server,
            Arc::clone(block_processor),
            bootstrap_initiator,
            ledger,
            id_str.to_string(),
            incremental_id,
            Arc::clone(connections),
            Arc::clone(workers),
            config.receive_minimum,
            Arc::clone(stats),
        )
        .unwrap(),
    ))))
}

#[no_mangle]
pub extern "C" fn rsn_bootstrap_attempt_wallet_run(handle: &BootstrapAttemptHandle) {
    let BootstrapStrategy::Wallet(wallet) = &***handle else {
        panic!("not wallet")
    };
    wallet.run();
}

#[no_mangle]
pub extern "C" fn rsn_bootstrap_attempt_wallet_size(handle: &BootstrapAttemptHandle) -> usize {
    let BootstrapStrategy::Wallet(wallet) = &***handle else {
        panic!("not wallet")
    };
    wallet.wallet_size()
}
