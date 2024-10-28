use crate::command_handler::RpcCommandHandler;
use rsnano_core::PublicKey;
use rsnano_rpc_messages::{AccountMoveArgs, MovedDto};

impl RpcCommandHandler {
    pub(crate) fn account_move(&self, args: AccountMoveArgs) -> anyhow::Result<MovedDto> {
        let public_keys: Vec<PublicKey> =
            args.accounts.iter().map(|account| account.into()).collect();

        self.node
            .wallets
            .move_accounts(&args.source, &args.wallet, &public_keys)?;

        Ok(MovedDto::new(true))
    }
}
