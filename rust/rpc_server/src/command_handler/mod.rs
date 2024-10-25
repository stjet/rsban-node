mod ledger;
mod node;
mod utils;
mod wallets;

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

    pub fn handle(&self, command: RpcCommand) -> RpcDto {
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
            RpcCommand::AccountsFrontiers(args) => self.accounts_frontiers(args),
            RpcCommand::WalletFrontiers(args) => self.wallet_frontiers(args),
            RpcCommand::Frontiers(args) => self.frontiers(args),
            RpcCommand::WalletInfo(args) => self.wallet_info(args),
            RpcCommand::WalletExport(args) => wallet_export(args),
            RpcCommand::PasswordChange(args) => self.password_change(args),
            RpcCommand::PasswordEnter(args) => self.password_enter(args),
            RpcCommand::PasswordValid(args) => self.password_valid(args),
            RpcCommand::DeterministicKey(args) => deterministic_key(args),
            RpcCommand::KeyExpand(args) => key_expand(args),
            RpcCommand::Peers(args) => self.peers(args),
            RpcCommand::PopulateBacklog => self.populate_backlog(),
            RpcCommand::Representatives(args) => self.representatives(args),
            RpcCommand::AccountsRepresentatives(args) => self.accounts_representatives(args),
            RpcCommand::StatsClear => self.stats_clear(),
            RpcCommand::UncheckedClear => self.unchecked_clear(),
            RpcCommand::Unopened(args) => self.unopened(args),
            RpcCommand::NodeId => self.node_id(),
            RpcCommand::Send(args) => self.send(args),
            RpcCommand::SearchReceivableAll => self.search_receivable_all(),
            RpcCommand::ReceiveMinimum => self.receive_minimum(),
            RpcCommand::WalletChangeSeed(args) => self.wallet_change_seed(args),
            RpcCommand::Delegators(args) => self.delegators(args),
            RpcCommand::DelegatorsCount(args) => self.delegators_count(args),
            RpcCommand::BlockHash(args) => block_hash(args),
            RpcCommand::AccountsBalances(args) => self.accounts_balances(args),
            RpcCommand::BlockInfo(args) => self.block_info(args),
            RpcCommand::Blocks(args) => self.blocks(args),
            RpcCommand::BlocksInfo(args) => self.blocks_info(args),
            RpcCommand::Chain(args) => self.chain(args, false),
            RpcCommand::Successors(args) => self.chain(args, true),
            RpcCommand::ConfirmationActive(args) => self.confirmation_active(args),
            RpcCommand::ConfirmationQuorum(args) => self.confirmation_quorum(args),
            RpcCommand::WorkValidate(args) => self.work_validate(args),
            RpcCommand::AccountInfo(args) => self.account_info(args),
            RpcCommand::AccountHistory(args) => self.account_history(args),
            RpcCommand::Sign(args) => self.sign(args),
            RpcCommand::Process(args) => self.process(args),
            RpcCommand::WorkCancel(args) => self.work_cancel(args),
            RpcCommand::Bootstrap(args) => self.bootstrap(args),
            RpcCommand::BootstrapAny(args) => self.bootstrap_any(args),
            RpcCommand::BoostrapLazy(args) => self.bootstrap_lazy(args),
            RpcCommand::WalletReceivable(args) => self.wallet_receivable(args),
            RpcCommand::WalletRepresentativeSet(args) => self.wallet_representative_set(args),
            RpcCommand::SearchReceivable(args) => self.search_receivable(args),
            RpcCommand::WalletRepublish(args) => self.wallet_republish(args),
            RpcCommand::WalletBalances(args) => self.wallet_balances(args),
            RpcCommand::WalletHistory(args) => self.wallet_history(args),
            RpcCommand::WalletLedger(args) => self.wallet_ledger(args),
            RpcCommand::AccountsReceivable(args) => self.accounts_receivable(args),
            RpcCommand::Receivable(args) => self.receivable(args),
            RpcCommand::ReceivableExists(args) => self.receivable_exists(args),
            RpcCommand::RepresentativesOnline(args) => self.representatives_online(args),
            RpcCommand::Unchecked(args) => self.unchecked(args),
            RpcCommand::UncheckedGet(args) => self.unchecked_get(args),
            RpcCommand::UncheckedKeys(args) => self.unchecked_keys(args),
            RpcCommand::ConfirmationInfo(args) => self.confirmation_info(args),
            RpcCommand::Ledger(args) => self.ledger(args),
            RpcCommand::WorkGenerate(args) => self.work_generate(args),
            RpcCommand::Republish(args) => self.republish(args),
            RpcCommand::BlockCreate(args) => self.block_create(args),
            RpcCommand::Telemetry(args) => self.telemetry(args),
        }
    }
}
