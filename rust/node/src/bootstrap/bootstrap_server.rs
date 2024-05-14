use crate::{
    stats::{DetailType, Direction, StatType, Stats},
    transport::{BufferDropPolicy, ChannelEnum, TrafficType},
    utils::ProcessingQueue,
};
use rsnano_core::{BlockEnum, BlockHash, Frontier};
use rsnano_ledger::Ledger;
use rsnano_messages::{
    AccountInfoAckPayload, AccountInfoReqPayload, AscPullAck, AscPullAckType, AscPullReq,
    AscPullReqType, BlocksAckPayload, BlocksReqPayload, FrontiersReqPayload, HashType, Message,
};
use rsnano_store_lmdb::{LmdbReadTransaction, Transaction};
use std::{
    cmp::min,
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};

/**
 * Processes bootstrap requests (`asc_pull_req` messages) and replies with bootstrap responses (`asc_pull_ack`)
 *
 * In order to ensure maximum throughput, there are two internal processing queues:
 * - One for doing ledger lookups and preparing responses (`request_queue`)
 * - One for sending back those responses over the network (`response_queue`)
 */
pub struct BootstrapServer {
    stats: Arc<Stats>,
    request_queue: ProcessingQueue<(AscPullReq, Arc<ChannelEnum>)>,
    on_response: Arc<Mutex<Option<Box<dyn Fn(&AscPullAck, &Arc<ChannelEnum>) + Send + Sync>>>>,
}

impl BootstrapServer {
    /** Maximum number of blocks to send in a single response, cannot be higher than capacity of a single `asc_pull_ack` message */
    const MAX_BLOCKS: usize = BlocksAckPayload::MAX_BLOCKS;
    const MAX_FRONTIERS: usize = AscPullAck::MAX_FRONTIERS;

    pub fn new(stats: Arc<Stats>, ledger: Arc<Ledger>) -> Self {
        let on_response = Arc::new(Mutex::new(None));
        let server_impl = BootstrapServerImpl {
            stats: Arc::clone(&stats),
            ledger,
            on_response: Arc::clone(&on_response),
        };
        Self {
            on_response,
            stats: Arc::clone(&stats),
            request_queue: ProcessingQueue::new(
                stats,
                StatType::BootstrapServer,
                "Bootstrap serv".to_string(),
                1,         // threads
                1024 * 16, //max size
                128,       // max batch
                Box::new(move |batch| server_impl.process_batch(batch)),
            ),
        }
    }

    pub fn start(&self) {
        self.request_queue.start();
    }

    pub fn stop(&self) {
        self.request_queue.stop();
    }

    pub fn set_response_callback(
        &self,
        cb: Box<dyn Fn(&AscPullAck, &Arc<ChannelEnum>) + Send + Sync>,
    ) {
        *self.on_response.lock().unwrap() = Some(cb);
    }
}

impl BootstrapServer {
    pub fn request(&self, message: AscPullReq, channel: Arc<ChannelEnum>) -> bool {
        if !self.verify(&message) {
            self.stats
                .inc(StatType::BootstrapServer, DetailType::Invalid);
            return false;
        }

        // If channel is full our response will be dropped anyway, so filter that early
        // TODO: Add per channel limits (this ideally should be done on the channel message processing side)
        if channel.max(TrafficType::Bootstrap) {
            self.stats.inc_dir(
                StatType::BootstrapServer,
                DetailType::ChannelFull,
                Direction::In,
            );
            return false;
        }

        self.request_queue.add((message, channel));
        return true;
    }

    fn verify(&self, message: &AscPullReq) -> bool {
        match &message.req_type {
            AscPullReqType::Blocks(i) => i.count > 0 && i.count as usize <= Self::MAX_BLOCKS,
            AscPullReqType::AccountInfo(i) => !i.target.is_zero(),
            AscPullReqType::Frontiers(i) => i.count > 0 && i.count as usize <= Self::MAX_FRONTIERS,
        }
    }
}

struct BootstrapServerImpl {
    stats: Arc<Stats>,
    ledger: Arc<Ledger>,
    on_response: Arc<Mutex<Option<Box<dyn Fn(&AscPullAck, &Arc<ChannelEnum>) + Send + Sync>>>>,
}

impl BootstrapServerImpl {
    fn process_batch(&self, batch: VecDeque<(AscPullReq, Arc<ChannelEnum>)>) {
        let mut tx = self.ledger.read_txn();
        for (request, channel) in batch {
            tx.refresh_if_needed(Duration::from_millis(500));

            if !channel.max(TrafficType::Bootstrap) {
                let response = self.process(&tx, request);
                self.respond(response, channel);
            } else {
                self.stats.inc_dir(
                    StatType::BootstrapServer,
                    DetailType::ChannelFull,
                    Direction::Out,
                );
            }
        }
    }

    fn process(&self, tx: &LmdbReadTransaction, message: AscPullReq) -> AscPullAck {
        match message.req_type {
            AscPullReqType::Blocks(blocks) => self.process_blocks(tx, message.id, blocks),
            AscPullReqType::AccountInfo(account) => self.process_account(tx, message.id, account),
            AscPullReqType::Frontiers(frontiers) => {
                self.process_frontiers(tx, message.id, frontiers)
            }
        }
    }

    fn process_blocks(
        &self,
        tx: &LmdbReadTransaction,
        id: u64,
        request: BlocksReqPayload,
    ) -> AscPullAck {
        let count = min(request.count as usize, BootstrapServer::MAX_BLOCKS);

        match request.start_type {
            HashType::Account => {
                if let Some(info) = self.ledger.account_info(tx, &request.start.into()) {
                    // Start from open block if pulling by account
                    return self.prepare_response(tx, id, info.open_block, count);
                }
            }
            HashType::Block => {
                if self.ledger.block_exists(tx, &request.start.into()) {
                    return self.prepare_response(tx, id, request.start.into(), count);
                }
            }
        }

        // Neither block nor account found, send empty response to indicate that
        self.prepare_empty_blocks_response(id)
    }

    /*
     * Account info request
     */

    fn process_account(
        &self,
        tx: &LmdbReadTransaction,
        id: u64,
        request: AccountInfoReqPayload,
    ) -> AscPullAck {
        let target = match request.target_type {
            HashType::Account => request.target.into(),
            HashType::Block => {
                // Try to lookup account assuming target is block hash
                self.ledger
                    .account(tx, &request.target.into())
                    .unwrap_or_default()
            }
        };

        let mut response_payload = AccountInfoAckPayload {
            account: target,
            ..Default::default()
        };

        if let Some(account_info) = self.ledger.account_info(tx, &target) {
            response_payload.account_open = account_info.open_block;
            response_payload.account_head = account_info.head;
            response_payload.account_block_count = account_info.block_count;

            if let Some(conf_info) = self.ledger.store.confirmation_height.get(tx, &target) {
                response_payload.account_conf_frontier = conf_info.frontier;
                response_payload.account_conf_height = conf_info.height;
            }
        }
        // If account is missing the response payload will contain all 0 fields, except for the target
        //
        AscPullAck {
            id,
            pull_type: AscPullAckType::AccountInfo(response_payload),
        }
    }

    /*
     * Frontiers request
     */
    fn process_frontiers(
        &self,
        tx: &LmdbReadTransaction,
        id: u64,
        request: FrontiersReqPayload,
    ) -> AscPullAck {
        let mut it = self.ledger.store.account.begin_account(tx, &request.start);
        let mut frontiers = Vec::new();
        while let Some((account, info)) = it.current() {
            frontiers.push(Frontier::new(*account, info.head));
            if frontiers.len() >= request.count as usize {
                break;
            }
            it.next();
        }

        AscPullAck {
            id,
            pull_type: AscPullAckType::Frontiers(frontiers),
        }
    }

    fn prepare_response(
        &self,
        tx: &LmdbReadTransaction,
        id: u64,
        start_block: BlockHash,
        count: usize,
    ) -> AscPullAck {
        let blocks = self.prepare_blocks(tx, start_block, count);
        let response_payload = BlocksAckPayload::new(blocks);

        AscPullAck {
            id,
            pull_type: AscPullAckType::Blocks(response_payload),
        }
    }

    fn prepare_empty_blocks_response(&self, id: u64) -> AscPullAck {
        AscPullAck {
            id,
            pull_type: AscPullAckType::Blocks(BlocksAckPayload::new(Vec::new())),
        }
    }

    fn prepare_blocks(
        &self,
        tx: &LmdbReadTransaction,
        start_block: BlockHash,
        count: usize,
    ) -> Vec<BlockEnum> {
        let mut result = Vec::new();
        if !start_block.is_zero() {
            let mut current = self.ledger.get_block(tx, &start_block);
            while let Some(c) = current.take() {
                let successor = c.sideband().unwrap().successor;
                result.push(c);

                if result.len() == count {
                    break;
                }
                current = self.ledger.get_block(tx, &successor);
            }
        }
        result
    }

    fn respond(&self, response: AscPullAck, channel: Arc<ChannelEnum>) {
        self.stats.inc_dir(
            StatType::BootstrapServer,
            DetailType::Response,
            Direction::Out,
        );

        // Increase relevant stats depending on payload type
        match &response.pull_type {
            AscPullAckType::Blocks(blocks) => {
                self.stats.inc_dir(
                    StatType::BootstrapServer,
                    DetailType::ResponseBlocks,
                    Direction::Out,
                );
                self.stats.add(
                    StatType::BootstrapServer,
                    DetailType::Blocks,
                    Direction::Out,
                    blocks.blocks().len() as u64,
                    false,
                );
            }
            AscPullAckType::AccountInfo(_) => {
                self.stats.inc_dir(
                    StatType::BootstrapServer,
                    DetailType::ResponseAccountInfo,
                    Direction::Out,
                );
            }
            AscPullAckType::Frontiers(frontiers) => {
                self.stats.inc_dir(
                    StatType::BootstrapServer,
                    DetailType::ResponseFrontiers,
                    Direction::Out,
                );
                self.stats.add(
                    StatType::BootstrapServer,
                    DetailType::Frontiers,
                    Direction::Out,
                    frontiers.len() as u64,
                    false,
                );
            }
        }

        {
            let callback = self.on_response.lock().unwrap();
            if let Some(cb) = &*callback {
                (cb)(&response, &channel);
            }
        }

        let msg = Message::AscPullAck(response);
        let stats = Arc::clone(&self.stats);
        channel.send(
            &msg,
            Some(Box::new(move |ec, _len| {
                if ec.is_err() {
                    stats.inc_dir(
                        StatType::BootstrapServer,
                        DetailType::WriteError,
                        Direction::Out,
                    );
                }
            })),
            BufferDropPolicy::Limiter,
            TrafficType::Bootstrap,
        );
    }
}
