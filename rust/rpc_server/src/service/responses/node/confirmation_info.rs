use std::sync::Arc;
use rsnano_core::{Amount, QualifiedRoot};
use rsnano_node::node::Node;
use rsnano_rpc_messages::{BlockInfoDto, ConfirmationInfoDto, ErrorDto};
use serde_json::to_string_pretty;
use std::collections::HashMap;

pub async fn confirmation_info(node: Arc<Node>, root: QualifiedRoot, contents: Option<bool>, representatives: Option<bool>) -> String {
    let election = node.active.election(&root);
    
    if let Some(election) = election {
        if !node.active.confirmed(&election) {
            let info = election.mutex.lock().unwrap();
            let mut blocks = HashMap::new();
            let mut total_tally = Amount::zero();

            for (hash, block) in info.last_blocks.iter() {
                let tally = info.last_tally.get(hash).cloned().unwrap_or(Amount::zero());
                let mut block_info = BlockInfoDto {
                    tally,
                    contents: None,
                    representatives: None,
                };

                if contents.unwrap_or(true) {
                    block_info.contents = Some(block.json_representation());
                }

                if representatives.unwrap_or(false) {
                    let mut reps = HashMap::new();
                    for (representative, vote) in &info.last_votes {
                        if hash == &vote.hash {
                            let amount = node.ledger.rep_weights.weight(representative);
                            reps.insert(representative.clone().into(), amount);
                        }
                    }
                    block_info.representatives = Some(reps);
                }

                total_tally += tally;
                blocks.insert(*hash, block_info);
            }

            let confirmation_info_dto = ConfirmationInfoDto::new(
                info.status.confirmation_request_count,
                info.last_votes.len(),
                info.status.winner.as_ref().map(|w| w.hash()).unwrap_or_default(),
                total_tally,
                info.status.final_tally,
                blocks,
            );

            to_string_pretty(&confirmation_info_dto).unwrap()
        } else {
            to_string_pretty(&ErrorDto::new("Confirmation not found".to_string())).unwrap()
        }
    } else {
        to_string_pretty(&ErrorDto::new("Invalid root".to_string())).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use rsnano_core::{Account, Amount, BlockBuilder, JsonBlock, DEV_GENESIS_KEY};
    use rsnano_ledger::DEV_GENESIS_HASH;
    use test_helpers::{assert_timely_msg, System};

    #[test]
    fn confirmation_info() {
        let mut system = System::new();
        let node = system.build_node().finish();

        let send = BlockBuilder::legacy_send()
            .previous(*DEV_GENESIS_HASH)
            .destination(Account::zero())
            .balance(Amount::MAX - Amount::raw(100))
            .sign((*DEV_GENESIS_KEY).clone())
            .work(node.work_generate_dev((*DEV_GENESIS_HASH).into()))
            .build();

        node.process_active(send.clone());

        assert_timely_msg(
            Duration::from_secs(5),
            || node.active.active(&send),
            "not active on node 1",
        );

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

        let root = send.qualified_root();

        let result = node.tokio.block_on(async {
            rpc_client
                .confirmation_info(root, Some(true), Some(true))
                .await
                .unwrap()
        });

        //assert_eq!(result.announcements, 1);
        assert_eq!(result.voters, 1);
        assert_eq!(result.last_winner, send.hash());

        let blocks = result.blocks;
        assert_eq!(blocks.len(), 1);

        let block = blocks.get(&send.hash()).unwrap();
        let representatives = block.representatives.clone().unwrap();
        assert_eq!(representatives.len(), 1);

        assert_eq!(result.total_tally, Amount::zero());

        let contents: &JsonBlock = block.contents.as_ref().unwrap();

        match contents {
            JsonBlock::Send(contents) => {
                assert_eq!(contents.previous, *DEV_GENESIS_HASH);
                assert_eq!(contents.destination, Account::zero());
                assert_eq!(contents.balance, Amount::MAX - Amount::raw(100));
            }
            _ => ()
        }

        server.abort();
    }
}

