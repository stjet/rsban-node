use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{
    ConfirmationEntry, ConfirmationHistoryArgs, ConfirmationHistoryResponse, ConfirmationStats,
};
use std::time::{Duration, UNIX_EPOCH};

impl RpcCommandHandler {
    pub(crate) fn confirmation_history(
        &self,
        args: ConfirmationHistoryArgs,
    ) -> ConfirmationHistoryResponse {
        let mut elections = Vec::new();
        let mut running_total = Duration::ZERO;
        let hash = args.hash.unwrap_or_default();
        for status in self.node.active.recently_cemented_list() {
            if hash.is_zero() || status.winner.as_ref().unwrap().hash() == hash {
                elections.push(ConfirmationEntry {
                    hash: status.winner.as_ref().unwrap().hash(),
                    duration: status.election_duration.as_secs(),
                    time: status
                        .election_end
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                    tally: status.tally,
                    final_tally: status.final_tally,
                    blocks: status.block_count,
                    voters: status.voter_count,
                    request_count: status.confirmation_request_count,
                });
            }
            running_total += status.election_duration;
        }

        ConfirmationHistoryResponse {
            confirmation_stats: ConfirmationStats {
                count: elections.len(),
                average: if elections.is_empty() {
                    None
                } else {
                    Some(running_total.as_secs() / elections.len() as u64)
                },
            },
            confirmations: elections,
        }
    }
}
