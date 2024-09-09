use std::sync::Arc;
use rsnano_core::{BlockDetails, BlockHash, Difficulty, DifficultyV1, WorkNonce, WorkVersion};
use rsnano_node::node::Node;
use rsnano_rpc_messages::WorkValidateDto;
use serde_json::to_string_pretty;

pub async fn work_validate(node: Arc<Node>, work: WorkNonce, hash: BlockHash) -> String {
    let result_difficulty = node.network_params.work.difficulty(WorkVersion::Work1, &hash.into(), work.into());

    let default_difficulty = DifficultyV1::default().get_difficulty(&hash.into(), work.into());

    let valid_all = result_difficulty >= default_difficulty;

    let receive_difficulty = node.network_params.work.threshold(
        &BlockDetails::new(rsnano_core::Epoch::Epoch2, false, true, false),
    );
    let valid_receive = result_difficulty >= receive_difficulty;

    let result_multiplier = DifficultyV1::to_multiplier(result_difficulty, default_difficulty);

    let work_validate_dto = WorkValidateDto {
        valid_all,
        valid_receive,
        difficulty: result_difficulty,
        multiplier: result_multiplier,
    };

    to_string_pretty(&work_validate_dto).unwrap()
}

#[cfg(test)]
mod tests {
    use crate::service::responses::test_helpers::setup_rpc_client_and_server;
    use rsnano_core::BlockHash;
    use test_helpers::System;

    #[test]
    fn work_validate() {
        let mut system = System::new();
        let node = system.make_node();

        let (rpc_client, server) = setup_rpc_client_and_server(node.clone(), true);

        let result = node
            .tokio
            .block_on(async { rpc_client.work_validate(1.into(), BlockHash::zero()).await.unwrap() });

        server.abort();
    }
}