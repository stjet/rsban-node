use crate::command_handler::RpcCommandHandler;
use anyhow::{anyhow, bail};
use rsnano_core::PendingKey;
use rsnano_node::wallets::WalletsExt;
use rsnano_rpc_messages::{BlockDto, ReceiveArgs};

impl RpcCommandHandler {
    pub fn receive(&self, args: ReceiveArgs) -> anyhow::Result<BlockDto> {
        self.ensure_control_enabled()?;

        let txn = self.node.ledger.read_txn();

        if !self.node.ledger.any().block_exists(&txn, &args.block) {
            bail!(Self::BLOCK_NOT_FOUND);
        }

        let pending_info = self
            .node
            .ledger
            .any()
            .get_pending(&txn, &PendingKey::new(args.account, args.block));
        if pending_info.is_none() {
            bail!("Block is not receivable");
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

        let _receive = self
            .node
            .wallets
            .receive_sync(
                wallet,
                &block,
                representative,
                self.node.config.receive_minimum,
            )
            .map_err(|_| anyhow!("Receive error"))?;

        Ok(BlockDto::new(block.hash()))
    }
}
