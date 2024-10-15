mod common;
//mod ledger;
//mod node;
//mod utils;
//mod wallets;

pub use common::*;
//pub use ledger::*;
//pub use node::*;
//pub use utils::*;
//pub use wallets::*;

use serde::{Deserialize, Serialize};

use crate::{AccountBlockCountDto, AccountHistoryDto, AccountInfoDto, AccountRepresentativeDto, AccountsRepresentativesDto, AccountsWithWorkDto, AvailableSupplyDto, BlockCountDto, BlockInfoDto, BlocksDto, BlocksInfoDto, ConfirmationActiveDto, ConfirmationQuorumDto, JsonDto, NodeIdDto, PeersDto, SignDto, UnopenedDto, WalletChangeSeedDto, WalletInfoDto, WalletRepresentativeDto, WalletRpcMessage, WorkDto, WorkValidateDto};

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RpcDto {
    AccountBalance(AccountBalanceDto),
    Account(AccountRpcMessage),
    Accounts(AccountsRpcMessage),
    Removed(RemovedDto),
    Moved(MovedDto),
    WalletCreate(WalletRpcMessage),
    KeyPair(KeyPairDto),
    Exists(ExistsDto),
    Error(ErrorDto2),
    Destroyed(DestroyedDto),
    Locked(LockedDto),
    Lock(LockedDto),
    Stop(SuccessDto),
    AccountBlockCount(AccountBlockCountDto),
    AccountKey(KeyRpcMessage),
    AccountGet(AccountRpcMessage),
    AccountRepresentative(AccountRepresentativeDto),
    AccountWeight(WeightDto),
    AvailableSupply(AvailableSupplyDto),
    BlockConfirm(StartedDto),
    BlockCount(BlockCountDto),
    BlockAccount(AccountRpcMessage),
    Uptime(UptimeDto),
    Keepalive(StartedDto),
    FrontierCount(CountRpcMessage),
    ValidateAccountNumber(SuccessDto),
    NanoToRaw(AmountRpcMessage),
    RawToNano(AmountRpcMessage),
    WalletAddWatch(SuccessDto),
    WalletRepresentative(WalletRepresentativeDto),
    WorkSet(SuccessDto),
    WorkGet(WorkDto),
    WalletWorkGet(AccountsWithWorkDto),
    AccountsFrontiers(FrontiersDto),
    WalletFrontiers(FrontiersDto),
    Frontiers(FrontiersDto),
    WalletInfo(WalletInfoDto),
    WalletExport(JsonDto),
    PasswordChange(SuccessDto),
    PasswordEnter(ValidDto),
    PasswordValid(ValidDto),
    DeterministicKey(KeyPairDto),
    KeyExpand(KeyPairDto),
    Peers(PeersDto),
    PopulateBacklog(SuccessDto),
    Representatives(RepresentativesDto),
    AccountsRepresentatives(AccountsRepresentativesDto),
    StatsClear(SuccessDto),
    UncheckedClear(SuccessDto),
    Unopened(UnopenedDto),
    NodeId(NodeIdDto),
    Send(BlockDto),
    SearchReceivableAll(SuccessDto),
    ReceiveMinimum(AmountRpcMessage),
    WalletChangeSeed(WalletChangeSeedDto),
    Delegators(DelegatorsDto),
    DelegatorsCount(CountRpcMessage),
    BlockHash(HashRpcMessage),
    AccountsBalances(AccountsBalancesDto),
    BlockInfo(BlockInfoDto),
    Blocks(BlocksDto),
    BlocksInfo(BlocksInfoDto),
    Chain(BlockHashesDto),
    ConfirmationActive(ConfirmationActiveDto),
    ConfirmationQuorum(ConfirmationQuorumDto),
    WorkValidate(WorkValidateDto),
    AccountInfo(AccountInfoDto),
    AccountHistory(AccountHistoryDto),
    Sign(SignDto),
    Process(HashRpcMessage),
    
}
