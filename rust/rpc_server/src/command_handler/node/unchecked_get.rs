use crate::command_handler::RpcCommandHandler;
use anyhow::anyhow;
use rsnano_core::{UncheckedInfo, UncheckedKey};
use rsnano_rpc_messages::{HashRpcMessage, UncheckedGetDto};
use std::sync::{Arc, Mutex};

impl RpcCommandHandler {
    pub(crate) fn unchecked_get(&self, args: HashRpcMessage) -> anyhow::Result<UncheckedGetDto> {
        let result = Arc::new(Mutex::new(None));

        self.node.unchecked.for_each(
            {
                let result = Arc::clone(&result);
                Box::new(move |key: &UncheckedKey, info: &UncheckedInfo| {
                    if key.hash == args.hash {
                        let modified_timestamp = info.modified;
                        if let Some(block) = info.block.as_ref() {
                            let contents = block.json_representation();
                            let mut result_guard = result.lock().unwrap();
                            *result_guard =
                                Some(UncheckedGetDto::new(modified_timestamp, contents));
                        }
                    }
                })
            },
            Box::new(|| true),
        );

        let result = result.lock().unwrap().take();
        result.ok_or_else(|| anyhow!(Self::BLOCK_NOT_FOUND))
    }
}
