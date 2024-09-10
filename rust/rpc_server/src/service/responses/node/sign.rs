use std::sync::Arc;
use rsnano_core::{sign_message, utils::MemoryStream, BlockEnum, PublicKey, RawKey};
use rsnano_node::node::Node;
use rsnano_rpc_messages::{ErrorDto, SignArgs, SignDto};
use serde_json::to_string_pretty;

pub async fn sign(node: Arc<Node>, args: SignArgs) -> String {
    let block: BlockEnum = args.block.into();

    let prv = if let Some(key) = args.key {
        key
    } else if let (Some(wallet), Some(account)) = (args.wallet, args.account) {
        node.wallets.fetch(&wallet, &account.into()).unwrap()
    } else {
        return to_string_pretty(&ErrorDto::new("Block create key required".to_string())).unwrap();
    };

    let signature = if prv != RawKey::zero() {
        let pub_key: PublicKey = (&prv).try_into().unwrap();
        let mut stream = MemoryStream::new();
        block.serialize(&mut stream);

        let signature = sign_message(&prv, &pub_key, &stream.to_vec());
        signature

    } else {
        return to_string_pretty(&ErrorDto::new("Block create key required".to_string())).unwrap();
    };

    let sign_dto = SignDto::new(signature, block.json_representation());

    to_string_pretty(&sign_dto).unwrap()
}

#[cfg(test)]
mod tests {
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use rsnano_core::{Amount, BlockEnum, StateBlock, DEV_GENESIS_KEY};
    use rsnano_ledger::{DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH, DEV_GENESIS_PUB_KEY};
    use test_helpers::System;

    #[test]
    fn sign() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), false);

        let send1 = BlockEnum::State(StateBlock::new(
            *DEV_GENESIS_ACCOUNT,
            *DEV_GENESIS_HASH,
            *DEV_GENESIS_PUB_KEY,
            Amount::MAX - Amount::raw(1),
            DEV_GENESIS_KEY.account().into(),
            &DEV_GENESIS_KEY,
            node.work_generate_dev((*DEV_GENESIS_HASH).into()),
        ));

        let result = node.tokio.block_on(async {
            rpc_client
                .sign(Some(DEV_GENESIS_KEY.private_key()), None, None, send1.json_representation())
                .await
                .unwrap()
        });

        server.abort();
    }
}