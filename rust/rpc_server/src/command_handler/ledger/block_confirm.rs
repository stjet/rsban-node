use crate::command_handler::RpcCommandHandler;
use rsnano_node::consensus::{ElectionStatus, ElectionStatusType};
use rsnano_rpc_messages::{ErrorDto, HashRpcMessage, RpcDto, StartedDto};
use std::sync::Arc;

impl RpcCommandHandler {
    pub(crate) fn block_confirm(&self, args: HashRpcMessage) -> RpcDto {
        let tx = self.node.ledger.read_txn();
        match &self.node.ledger.any().get_block(&tx, &args.hash) {
            Some(block) => {
                if !self
                    .node
                    .ledger
                    .confirmed()
                    .block_exists_or_pruned(&tx, &args.hash)
                {
                    if !self.node.confirming_set.exists(&args.hash) {
                        self.node
                            .election_schedulers
                            .manual
                            .push(Arc::new(block.clone()), None);
                    }
                } else {
                    let mut status = ElectionStatus::default();
                    status.winner = Some(Arc::new(block.clone()));
                    status.election_end = std::time::SystemTime::now();
                    status.block_count = 1;
                    status.election_status_type = ElectionStatusType::ActiveConfirmationHeight;
                    self.node.active.insert_recently_cemented(status);
                }
                RpcDto::BlockConfirm(StartedDto::new(true))
            }
            None => RpcDto::Error(ErrorDto::BlockNotFound),
        }
    }
}
