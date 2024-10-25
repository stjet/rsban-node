use crate::command_handler::RpcCommandHandler;
use rsnano_core::PendingKey;
use rsnano_node::wallets::WalletsExt;
use rsnano_rpc_messages::{BlockDto, ErrorDto, ReceiveArgs, RpcDto};

impl RpcCommandHandler {
    pub fn receive(&self, args: ReceiveArgs) -> RpcDto {
        if !self.enable_control {
            return RpcDto::Error(ErrorDto::RPCControlDisabled);
        }

        let txn = self.node.ledger.read_txn();

        if !self.node.ledger.any().block_exists(&txn, &args.block) {
            return RpcDto::Error(ErrorDto::BlockNotFound);
        }

        let pending_info = self
            .node
            .ledger
            .any()
            .get_pending(&txn, &PendingKey::new(args.account, args.block));
        if pending_info.is_none() {
            return RpcDto::Error(ErrorDto::BlockNotReceivable);
        }

        let representative = self
            .node
            .wallets
            .get_representative(args.wallet)
            .unwrap_or_default();

        let wallets = self.node.wallets.mutex.lock().unwrap();
        let wallet = wallets.get(&args.wallet).unwrap().to_owned();

        let block = self
            .node
            .ledger
            .any()
            .get_block(&self.node.ledger.read_txn(), &args.block)
            .unwrap();

        let receive = self.node.wallets.receive_sync(
            wallet,
            &block,
            representative,
            self.node.config.receive_minimum,
        );

        match receive {
            Ok(_) => RpcDto::Receive(BlockDto::new(block.hash())),
            Err(_) => RpcDto::Error(ErrorDto::ReceiveError),
        }
    }
}
