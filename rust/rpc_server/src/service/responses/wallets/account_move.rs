use rsnano_core::PublicKey;
use rsnano_node::Node;
use rsnano_rpc_messages::{AccountMoveArgs, ErrorDto2, MovedDto};
use std::sync::Arc;

use crate::RpcResult;

pub async fn account_move(
    node: Arc<Node>,
    enable_control: bool,
    args: AccountMoveArgs
) -> RpcResult<MovedDto> {
    if enable_control {
        let public_keys: Vec<PublicKey> = args.accounts.iter().map(|account| account.into()).collect();
        let result = node.wallets.move_accounts(&args.source, &args.wallet, &public_keys);

        match result {
            Ok(()) => RpcResult::Ok(MovedDto::new(true)),
            Err(e) => RpcResult::Err(ErrorDto2::WalletsError(e)),
        }
    } else {
        RpcResult::Err(ErrorDto2::RPCControlDisabled)
    }
}
