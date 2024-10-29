use crate::command_handler::RpcCommandHandler;
use indexmap::IndexMap;
use rsnano_core::{Account, Amount};
use rsnano_rpc_messages::{RepresentativesArgs, RepresentativesResponse};

impl RpcCommandHandler {
    pub(crate) fn representatives(&self, args: RepresentativesArgs) -> RepresentativesResponse {
        let mut representatives: IndexMap<Account, Amount> = self
            .node
            .ledger
            .rep_weights
            .read()
            .iter()
            .map(|(pk, amount)| (Account::from(pk), *amount))
            .collect();

        if args.sorting.unwrap_or(false) {
            representatives.sort_by(|_, v1, _, v2| v2.cmp(v1));
        }

        let count = args.count.unwrap_or(std::u64::MAX);
        while representatives.len() as u64 > count {
            representatives.pop();
        }

        RepresentativesResponse::new(representatives)
    }
}
