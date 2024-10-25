use crate::*;
use rsnano_node::Node;
use rsnano_rpc_messages::{RpcCommand, RpcDto};
use std::sync::Arc;

#[derive(Clone)]
struct RpcService {
    node: Arc<Node>,
    enable_control: bool,
}

#[derive(Clone)]
pub(crate) struct RpcCommandHandler {
    service: RpcService,
}

impl RpcCommandHandler {
    pub fn new(node: Arc<Node>, enable_control: bool) -> Self {
        let service = RpcService {
            node,
            enable_control,
        };
        Self { service }
    }

    pub async fn handle(&self, command: RpcCommand) -> RpcDto {
        let node = self.service.node.clone();
        let enable_control = self.service.enable_control;
        match command {
            RpcCommand::WorkPeers => work_peers(node, enable_control).await,
            RpcCommand::WorkPeerAdd(args) => work_peer_add(node, enable_control, args).await,
            RpcCommand::WorkPeersClear => work_peers_clear(node, enable_control).await,
            RpcCommand::Receive(args) => receive(node, enable_control, args).await,
            RpcCommand::AccountCreate(args) => account_create(node, enable_control, args).await,
            RpcCommand::AccountBalance(args) => account_balance(node, args).await,
            RpcCommand::AccountsCreate(args) => accounts_create(node, enable_control, args).await,
            RpcCommand::AccountRemove(args) => account_remove(node, enable_control, args).await,
            RpcCommand::AccountMove(args) => account_move(node, enable_control, args).await,
            RpcCommand::AccountList(args) => account_list(node, args).await,
            RpcCommand::WalletCreate(args) => wallet_create(node, enable_control, args).await,
            RpcCommand::KeyCreate => key_create().await,
            RpcCommand::WalletAdd(args) => wallet_add(node, enable_control, args).await,
            RpcCommand::WalletContains(args) => wallet_contains(node, args).await,
            RpcCommand::WalletDestroy(args) => wallet_destroy(node, enable_control, args).await,
            RpcCommand::WalletLock(args) => wallet_lock(node, enable_control, args).await,
            RpcCommand::WalletLocked(args) => wallet_locked(node, args).await,
            RpcCommand::Stop => stop(node, enable_control).await,
            RpcCommand::AccountBlockCount(args) => account_block_count(node, args).await,
            RpcCommand::AccountKey(args) => account_key(args).await,
            RpcCommand::AccountGet(args) => account_get(args).await,
            RpcCommand::AccountRepresentative(args) => account_representative(node, args).await,
            RpcCommand::AccountWeight(args) => account_weight(node, args).await,
            RpcCommand::AvailableSupply => available_supply(node).await,
            RpcCommand::BlockConfirm(args) => block_confirm(node, args).await,
            RpcCommand::BlockCount => block_count(node).await,
            RpcCommand::BlockAccount(args) => block_account(node, args).await,
            RpcCommand::Uptime => uptime(node).await,
            RpcCommand::Keepalive(args) => keepalive(node, enable_control, args).await,
            RpcCommand::FrontierCount => frontier_count(node).await,
            RpcCommand::ValidateAccountNumber(_) => validate_account_number().await,
            RpcCommand::NanoToRaw(args) => nano_to_raw(args).await,
            RpcCommand::RawToNano(args) => raw_to_nano(args).await,
            RpcCommand::WalletAddWatch(args) => wallet_add_watch(node, enable_control, args).await,
            RpcCommand::WalletRepresentative(args) => wallet_representative(node, args).await,
            RpcCommand::WorkSet(args) => work_set(node, enable_control, args).await,
            RpcCommand::WorkGet(args) => work_get(node, enable_control, args).await,
            RpcCommand::WalletWorkGet(args) => wallet_work_get(node, enable_control, args).await,
            RpcCommand::AccountsFrontiers(args) => accounts_frontiers(node, args).await,
            RpcCommand::WalletFrontiers(args) => wallet_frontiers(node, args).await,
            RpcCommand::Frontiers(args) => frontiers(node, args).await,
            RpcCommand::WalletInfo(args) => wallet_info(node, args).await,
            RpcCommand::WalletExport(args) => wallet_export(args).await,
            RpcCommand::PasswordChange(args) => password_change(node, enable_control, args).await,
            RpcCommand::PasswordEnter(args) => password_enter(node, args).await,
            RpcCommand::PasswordValid(args) => password_valid(node, args).await,
            RpcCommand::DeterministicKey(args) => deterministic_key(args).await,
            RpcCommand::KeyExpand(args) => key_expand(args).await,
            RpcCommand::Peers(args) => peers(node, args).await,
            RpcCommand::PopulateBacklog => populate_backlog(node).await,
            RpcCommand::Representatives(args) => representatives(node, args).await,
            RpcCommand::AccountsRepresentatives(args) => accounts_representatives(node, args).await,
            RpcCommand::StatsClear => stats_clear(node).await,
            RpcCommand::UncheckedClear => unchecked_clear(node).await,
            RpcCommand::Unopened(args) => unopened(node, enable_control, args).await,
            RpcCommand::NodeId => node_id(node, enable_control).await,
            RpcCommand::Send(args) => send(node, enable_control, args).await,
            RpcCommand::SearchReceivableAll => search_receivable_all(node, enable_control).await,
            RpcCommand::ReceiveMinimum => receive_minimum(node, enable_control).await,
            RpcCommand::WalletChangeSeed(args) => {
                wallet_change_seed(node, enable_control, args).await
            }
            RpcCommand::Delegators(args) => delegators(node, args).await,
            RpcCommand::DelegatorsCount(args) => delegators_count(node, args).await,
            RpcCommand::BlockHash(args) => block_hash(args).await,
            RpcCommand::AccountsBalances(args) => accounts_balances(node, args).await,
            RpcCommand::BlockInfo(args) => block_info(node, args).await,
            RpcCommand::Blocks(args) => blocks(node, args).await,
            RpcCommand::BlocksInfo(args) => blocks_info(node, args).await,
            RpcCommand::Chain(args) => chain(node, args, false).await,
            RpcCommand::Successors(args) => chain(node, args, true).await,
            RpcCommand::ConfirmationActive(args) => confirmation_active(node, args).await,
            RpcCommand::ConfirmationQuorum(args) => confirmation_quorum(node, args).await,
            RpcCommand::WorkValidate(args) => work_validate(node, args).await,
            RpcCommand::AccountInfo(args) => account_info(node, args).await,
            RpcCommand::AccountHistory(args) => account_history(node, args).await,
            RpcCommand::Sign(args) => sign(node, args).await,
            RpcCommand::Process(args) => process(node, args).await,
            RpcCommand::WorkCancel(args) => work_cancel(node, enable_control, args).await,
            RpcCommand::Bootstrap(args) => bootstrap(node, args).await,
            RpcCommand::BootstrapAny(args) => bootstrap_any(node, args).await,
            RpcCommand::BoostrapLazy(args) => bootstrap_lazy(node, args).await,
            RpcCommand::WalletReceivable(args) => {
                wallet_receivable(node, enable_control, args).await
            }
            RpcCommand::WalletRepresentativeSet(args) => {
                wallet_representative_set(node, enable_control, args).await
            }
            RpcCommand::SearchReceivable(args) => {
                search_receivable(node, enable_control, args).await
            }
            RpcCommand::WalletRepublish(args) => wallet_republish(node, enable_control, args).await,
            RpcCommand::WalletBalances(args) => wallet_balances(node, args).await,
            RpcCommand::WalletHistory(args) => wallet_history(node, args).await,
            RpcCommand::WalletLedger(args) => wallet_ledger(node, enable_control, args).await,
            RpcCommand::AccountsReceivable(args) => accounts_receivable(node, args).await,
            RpcCommand::Receivable(args) => receivable(node, args).await,
            RpcCommand::ReceivableExists(args) => receivable_exists(node, args).await,
            RpcCommand::RepresentativesOnline(args) => representatives_online(node, args).await,
            RpcCommand::Unchecked(args) => unchecked(node, args).await,
            RpcCommand::UncheckedGet(args) => unchecked_get(node, args).await,
            RpcCommand::UncheckedKeys(args) => unchecked_keys(node, args).await,
            RpcCommand::ConfirmationInfo(args) => confirmation_info(node, args).await,
            RpcCommand::Ledger(args) => ledger(node, enable_control, args).await,
            RpcCommand::WorkGenerate(args) => work_generate(node, enable_control, args).await,
            RpcCommand::Republish(args) => republish(node, args).await,
            RpcCommand::BlockCreate(args) => block_create(node, enable_control, args).await,
            RpcCommand::Telemetry(args) => telemetry(node, args).await,
        }
    }
}
