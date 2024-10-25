use crate::command_handler::RpcCommandHandler;
use anyhow::{anyhow, bail};
use rsnano_core::Amount;
use rsnano_rpc_messages::{ConfirmationBlockInfoDto, ConfirmationInfoArgs, ConfirmationInfoDto};
use std::collections::HashMap;

impl RpcCommandHandler {
    pub(crate) fn confirmation_info(
        &self,
        args: ConfirmationInfoArgs,
    ) -> anyhow::Result<ConfirmationInfoDto> {
        let election = self
            .node
            .active
            .election(&args.root)
            .ok_or_else(|| anyhow!("Invalid root"))?;

        if !self.node.active.confirmed(&election) {
            bail!("Confirmation info not found");
        }

        let info = election.mutex.lock().unwrap();
        let mut blocks = HashMap::new();
        let mut total_tally = Amount::zero();

        for (hash, block) in info.last_blocks.iter() {
            let tally = info.last_tally.get(hash).cloned().unwrap_or(Amount::zero());
            let mut block_info = ConfirmationBlockInfoDto {
                tally,
                contents: None,
                representatives: None,
            };

            if args.contents.unwrap_or(true) {
                block_info.contents = Some(block.json_representation());
            }

            if args.representatives.unwrap_or(false) {
                let mut reps = HashMap::new();
                for (representative, vote) in &info.last_votes {
                    if hash == &vote.hash {
                        let amount = self.node.ledger.rep_weights.weight(representative);
                        reps.insert(representative.clone().into(), amount);
                    }
                }
                block_info.representatives = Some(reps);
            }

            total_tally += tally;
            blocks.insert(*hash, block_info);
        }

        Ok(ConfirmationInfoDto::new(
            info.status.confirmation_request_count,
            info.last_votes.len(),
            info.status
                .winner
                .as_ref()
                .map(|w| w.hash())
                .unwrap_or_default(),
            total_tally,
            info.status.final_tally,
            blocks,
        ))
    }
}
