use crate::command_handler::RpcCommandHandler;
use rsnano_rpc_messages::{ConfirmationActiveArgs, ConfirmationActiveDto, RpcDto};

impl RpcCommandHandler {
    pub(crate) fn confirmation_active(&self, args: ConfirmationActiveArgs) -> RpcDto {
        let announcements = args.announcements.unwrap_or(0);
        let mut confirmed = 0;
        let mut confirmations = Vec::new();

        let active_elections = self.node.active.list_active(usize::MAX);
        for election in active_elections {
            if election
                .confirmation_request_count
                .load(std::sync::atomic::Ordering::Relaxed) as u64
                >= announcements
            {
                if !self.node.active.confirmed(&election) {
                    confirmations.push(election.qualified_root.clone());
                } else {
                    confirmed += 1;
                }
            }
        }

        let unconfirmed = confirmations.len() as u64;

        let confirmation_active_dto =
            ConfirmationActiveDto::new(confirmations, unconfirmed, confirmed);

        RpcDto::ConfirmationActive(confirmation_active_dto)
    }
}
