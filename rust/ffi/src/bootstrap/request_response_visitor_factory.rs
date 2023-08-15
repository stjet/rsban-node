use crate::{
    block_processing::BlockProcessorHandle,
    ledger::datastore::LedgerHandle,
    transport::SynCookiesHandle,
    utils::{LoggerHandle, LoggerMT, ThreadPoolHandle},
    NetworkParamsDto, NodeConfigDto, NodeFlagsHandle, StatHandle,
};
use rsnano_core::{utils::Logger, KeyPair};

use rsnano_node::{bootstrap::RequestResponseVisitorFactory, config::NodeConfig, NetworkParams};
use std::sync::Arc;

use super::bootstrap_initiator::BootstrapInitiatorHandle;

pub struct RequestResponseVisitorFactoryHandle(pub Arc<RequestResponseVisitorFactory>);

#[repr(C)]
pub struct RequestResponseVisitorFactoryParams {
    pub config: *const NodeConfigDto,
    pub logger: *mut LoggerHandle,
    pub workers: *mut ThreadPoolHandle,
    pub network: *const NetworkParamsDto,
    pub stats: *mut StatHandle,
    pub syn_cookies: *mut SynCookiesHandle,
    pub node_id_prv: *const u8,
    pub ledger: *mut LedgerHandle,
    pub block_processor: *mut BlockProcessorHandle,
    pub bootstrap_initiator: *mut BootstrapInitiatorHandle,
    pub flags: *const NodeFlagsHandle,
}

#[no_mangle]
pub unsafe extern "C" fn rsn_request_response_visitor_factory_create(
    params: &RequestResponseVisitorFactoryParams,
) -> *mut RequestResponseVisitorFactoryHandle {
    let config = Arc::new(NodeConfig::try_from(&*params.config).unwrap());
    let logger: Arc<dyn Logger> = Arc::new(LoggerMT::new(Box::from_raw(params.logger)));
    let workers = (*params.workers).0.clone();
    let network = NetworkParams::try_from(&*params.network).unwrap();
    let stats = Arc::clone(&(*params.stats));
    let ledger = Arc::clone(&(*params.ledger));
    let block_processor = Arc::clone(&(*params.block_processor));
    let bootstrap_initiator = Arc::clone(&(*params.bootstrap_initiator));
    let node_flags = (*params.flags).0.lock().unwrap().clone();
    let logging_config = config.logging.clone();
    let node_id = Arc::new(
        KeyPair::from_priv_key_bytes(std::slice::from_raw_parts(params.node_id_prv, 32)).unwrap(),
    );
    let mut visitor_factory = RequestResponseVisitorFactory::new(
        Arc::clone(&logger),
        Arc::clone(&*params.syn_cookies),
        Arc::clone(&stats),
        network.network.clone(),
        node_id,
        ledger,
        workers,
        block_processor,
        bootstrap_initiator,
        node_flags,
        logging_config,
    );
    visitor_factory.handshake_logging = config.logging.network_node_id_handshake_logging_value;
    Box::into_raw(Box::new(RequestResponseVisitorFactoryHandle(Arc::new(
        visitor_factory,
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_request_response_visitor_factory_destroy(
    handle: *mut RequestResponseVisitorFactoryHandle,
) {
    drop(Box::from_raw(handle))
}
