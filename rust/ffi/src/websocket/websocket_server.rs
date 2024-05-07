use super::WebsocketListenerHandle;
use crate::{
    consensus::{ActiveTransactionsHandle, VoteProcessorHandle},
    telemetry::TelemetryHandle,
    utils::AsyncRuntimeHandle,
    wallets::LmdbWalletsHandle,
    WebsocketConfigDto,
};
use rsnano_node::websocket::create_websocket_server;
use std::sync::Arc;

#[no_mangle]
pub extern "C" fn rsn_websocket_server_create(
    config: &WebsocketConfigDto,
    wallets: &LmdbWalletsHandle,
    async_rt: &AsyncRuntimeHandle,
    active_transactions: &ActiveTransactionsHandle,
    telemetry: &TelemetryHandle,
    vote_processor: &VoteProcessorHandle,
) -> *mut WebsocketListenerHandle {
    match create_websocket_server(
        config.into(),
        Arc::clone(wallets),
        Arc::clone(async_rt),
        active_transactions,
        telemetry,
        vote_processor,
    ) {
        Some(listener) => WebsocketListenerHandle::new(listener),
        None => std::ptr::null_mut(),
    }
}
