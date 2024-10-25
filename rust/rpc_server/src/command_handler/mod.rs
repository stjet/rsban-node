mod ledger;
mod node;
mod utils;
mod wallets;

use ledger::*;
use node::*;
use rsnano_node::Node;
use rsnano_rpc_messages::{RpcCommand, RpcDto};
use std::sync::Arc;
use utils::*;
use wallets::*;

#[derive(Clone)]
pub(crate) struct RpcCommandHandler {
    node: Arc<Node>,
    enable_control: bool,
}

impl RpcCommandHandler {
    pub fn new(node: Arc<Node>, enable_control: bool) -> Self {
        Self {
            node,
            enable_control,
        }
    }

    pub async fn handle(&self, command: RpcCommand) -> RpcDto {
        let node = self.node.clone();
        let enable_control = self.enable_control;
        match command {
            RpcCommand::WorkPeers => self.work_peers(),
            RpcCommand::WorkPeerAdd(args) => self.work_peer_add(args),
            RpcCommand::WorkPeersClear => self.work_peers_clear(),
            RpcCommand::Receive(args) => self.receive(args),
            RpcCommand::AccountCreate(args) => self.account_create(args),
            RpcCommand::AccountBalance(args) => self.account_balance(args),
            RpcCommand::AccountsCreate(args) => self.accounts_create(args),
            RpcCommand::AccountRemove(args) => self.account_remove(args),
            RpcCommand::AccountMove(args) => self.account_move(args),
            RpcCommand::AccountList(args) => self.account_list(args),
            RpcCommand::WalletCreate(args) => self.wallet_create(args),
            RpcCommand::KeyCreate => key_create(),
            RpcCommand::WalletAdd(args) => self.wallet_add(args),
            RpcCommand::WalletContains(args) => self.wallet_contains(args),
            RpcCommand::WalletDestroy(args) => self.wallet_destroy(args),
            RpcCommand::WalletLock(args) => self.wallet_lock(args),
            RpcCommand::WalletLocked(args) => self.wallet_locked(args),
            RpcCommand::Stop => self.stop(),
            RpcCommand::AccountBlockCount(args) => self.account_block_count(args),
            RpcCommand::AccountKey(args) => account_key(args),
            RpcCommand::AccountGet(args) => account_get(args),
            RpcCommand::AccountRepresentative(args) => self.account_representative(args),
            RpcCommand::AccountWeight(args) => self.account_weight(args),
            RpcCommand::AvailableSupply => self.available_supply(),
            RpcCommand::BlockConfirm(args) => self.block_confirm(args),
            RpcCommand::BlockCount => self.block_count(),
            RpcCommand::BlockAccount(args) => self.block_account(args),
            RpcCommand::Uptime => self.uptime(),
            RpcCommand::Keepalive(args) => self.keepalive(args),
            RpcCommand::FrontierCount => self.frontier_count(),
            RpcCommand::ValidateAccountNumber(_args) => validate_account_number("TODO".to_string()),
            RpcCommand::NanoToRaw(args) => nano_to_raw(args),
            RpcCommand::RawToNano(args) => raw_to_nano(args),
            RpcCommand::WalletAddWatch(args) => self.wallet_add_watch(args),
            RpcCommand::WalletRepresentative(args) => self.wallet_representative(args),
            RpcCommand::WorkSet(args) => self.work_set(args),
            RpcCommand::WorkGet(args) => self.work_get(args),
            RpcCommand::WalletWorkGet(args) => self.wallet_work_get(args),
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
