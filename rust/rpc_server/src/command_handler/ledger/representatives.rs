use crate::command_handler::RpcCommandHandler;
use rsnano_core::{Account, Amount};
use rsnano_rpc_messages::{RepresentativesArgs, RepresentativesDto};
use std::collections::HashMap;

impl RpcCommandHandler {
    pub(crate) fn representatives(&self, args: RepresentativesArgs) -> RepresentativesDto {
        let mut representatives: Vec<(Account, Amount)> = self
            .node
            .ledger
            .rep_weights
            .read()
            .iter()
            .map(|(pk, amount)| (Account::from(pk), *amount))
            .collect();

        if args.sorting.unwrap_or(false) {
            representatives.sort_by(|a, b| b.1.cmp(&a.1));
        }

        let count = args.count.unwrap_or(std::u64::MAX);
        let limited_representatives: HashMap<Account, Amount> =
            representatives.into_iter().take(count as usize).collect();

        RepresentativesDto::new(limited_representatives)
    }
}
