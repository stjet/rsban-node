use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{ConfirmationActiveArgs, ConfirmationActiveResponse};

impl RpcCommandHandler {
    pub(crate) fn confirmation_active(
        &self,
        args: ConfirmationActiveArgs,
    ) -> ConfirmationActiveResponse {
        let announcements = args.announcements.unwrap_or(0);
        let mut confirmed = 0;
        let mut elections = Vec::new();

        let active_elections = self.node.active.list_active(usize::MAX);
        for election in active_elections {
            if election
                .confirmation_request_count
                .load(std::sync::atomic::Ordering::Relaxed) as u64
                >= announcements
            {
                if !self.node.active.confirmed(&election) {
                    elections.push(election.qualified_root.clone());
                } else {
                    confirmed += 1;
                }
            }
        }

        let unconfirmed = elections.len() as u64;
        ConfirmationActiveResponse {
            confirmations: elections,
            unconfirmed,
            confirmed,
        }
    }
}
