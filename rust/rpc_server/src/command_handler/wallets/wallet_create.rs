use crate::command_handler::RpcCommandHandler;
use rsnano_core::WalletId;
use rsnano_node::wallets::WalletsExt;
use rsnano_rpc_messages::{ErrorDto, RpcDto, WalletCreateArgs, WalletRpcMessage};

impl RpcCommandHandler {
    pub(crate) fn wallet_create(&self, args: WalletCreateArgs) -> RpcDto {
        if !self.enable_control {
            return RpcDto::Error(ErrorDto::RPCControlDisabled);
        }

        let wallet = WalletId::random();
        self.node.wallets.create(wallet);
        let wallet_create_dto = WalletRpcMessage::new(wallet);

        if let Some(seed) = args.seed {
            self.node
                .wallets
                .change_seed(wallet, &seed, 0)
                .expect("This should not fail since the wallet was just created");
        }

        RpcDto::WalletCreate(wallet_create_dto)
    }
}
