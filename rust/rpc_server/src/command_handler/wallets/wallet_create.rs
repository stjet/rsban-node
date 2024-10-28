use crate::command_handler::RpcCommandHandler;
use rsnano_core::WalletId;
use rsnano_node::wallets::WalletsExt;
use rsnano_rpc_messages::{WalletCreateArgs, WalletRpcMessage};

impl RpcCommandHandler {
    pub(crate) fn wallet_create(&self, args: WalletCreateArgs) -> WalletRpcMessage {
        let wallet = WalletId::random();
        self.node.wallets.create(wallet);

        if let Some(seed) = args.seed {
            self.node
                .wallets
                .change_seed(wallet, &seed, 0)
                .expect("This should not fail since the wallet was just created");
        }
        WalletRpcMessage::new(wallet)
    }
}
