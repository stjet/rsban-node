use crate::command_handler::RpcCommandHandler;
use anyhow::{anyhow, bail};
use rsnano_core::BlockEnum;
use rsnano_ledger::BlockStatus;
use rsnano_rpc_messages::{HashRpcMessage, ProcessArgs};
use std::sync::Arc;

impl RpcCommandHandler {
    pub(crate) fn process(&self, args: ProcessArgs) -> anyhow::Result<HashRpcMessage> {
        let is_async = args.is_async.unwrap_or(false);
        let block: BlockEnum = args.block.into();

        if self.node.network_params.work.validate_entry(
            block.work_version(),
            &block.root(),
            block.work(),
        ) {
            bail!("Work low");
        }

        if !is_async {
            match self.node.process_local(block.clone()) {
                Some(result) => match result {
                    BlockStatus::Progress => {
                        let hash = block.hash();
                        Ok(HashRpcMessage::new(hash))
                    }
                    BlockStatus::Fork => {
                        if args.force.unwrap_or(false) {
                            self.node.active.erase(&block.qualified_root());
                            self.node.block_processor.force(Arc::new(block.clone()));
                            let hash = block.hash();
                            Ok(HashRpcMessage::new(hash))
                        } else {
                            Err(anyhow!(result.as_str()))
                        }
                    }
                    _ => Err(anyhow!(result.as_str())),
                },
                None => Err(anyhow!("Stopped")),
            }
        } else {
            if let BlockEnum::State(_) = block {
                self.node.process(block.clone()).unwrap(); // TODO add error handling!
                Ok(HashRpcMessage::new(block.hash()))
            } else {
                Err(anyhow!(Self::BLOCK_ERROR))
            }
        }
    }
}
