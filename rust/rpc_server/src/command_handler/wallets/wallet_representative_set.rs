use crate::command_handler::RpcCommandHandler;
use rsnano_node::wallets::WalletsExt;
use rsnano_rpc_messages::{SetDto, WalletRepresentativeSetArgs};

impl RpcCommandHandler {
    pub(crate) fn wallet_representative_set(
        &self,
        args: WalletRepresentativeSetArgs,
    ) -> anyhow::Result<SetDto> {
        self.ensure_control_enabled()?;
        let update_existing = args.update_existing_accounts.unwrap_or(false);
        self.node
            .wallets
            .set_representative(args.wallet, args.account.into(), update_existing)?;
        Ok(SetDto::new(true))
    }
}
