use crate::command_handler::RpcCommandHandler;
use rsnano_core::{BlockDetails, DifficultyV1, WorkVersion};
use rsnano_rpc_messages::{RpcDto, WorkValidateArgs, WorkValidateDto};

impl RpcCommandHandler {
    pub(crate) fn work_validate(&self, args: WorkValidateArgs) -> WorkValidateDto {
        let result_difficulty = self.node.network_params.work.difficulty(
            WorkVersion::Work1,
            &args.hash.into(),
            args.work.into(),
        );

        let default_difficulty = self
            .node
            .network_params
            .work
            .threshold_base(WorkVersion::Work1);

        let valid_all = result_difficulty >= default_difficulty;

        let receive_difficulty = self.node.network_params.work.threshold(&BlockDetails::new(
            rsnano_core::Epoch::Epoch2,
            false,
            true,
            false,
        ));
        let valid_receive = result_difficulty >= receive_difficulty;

        let result_multiplier = DifficultyV1::to_multiplier(result_difficulty, default_difficulty);

        WorkValidateDto {
            valid_all,
            valid_receive,
            difficulty: result_difficulty,
            multiplier: result_multiplier,
        }
    }
}
