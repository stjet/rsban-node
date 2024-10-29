mod ledger;
mod node;
mod utils;
mod wallets;

use anyhow::anyhow;
use rsnano_core::{Account, AccountInfo, BlockEnum, BlockHash};
use rsnano_node::Node;
use rsnano_rpc_messages::{RpcCommand, RpcError, StatsType};
use rsnano_store_lmdb::Transaction;
use serde_json::{to_value, Value};
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

    pub fn handle(&self, command: RpcCommand) -> serde_json::Value {
        self.call_handler(command).unwrap_or_else(Self::error_value)
    }

    fn error_value(error: anyhow::Error) -> serde_json::Value {
        serde_json::to_value(RpcError::new(error.to_string())).unwrap()
    }

    fn call_handler(&self, command: RpcCommand) -> anyhow::Result<serde_json::Value> {
        self.check_control_enabled(&command)?;
        let response = match command {
            RpcCommand::AccountBalance(args) => to_value(self.account_balance(args)),
            RpcCommand::AccountBlockCount(args) => to_value(self.account_block_count(args)?),
            RpcCommand::AccountCreate(args) => to_value(self.account_create(args)?),
            RpcCommand::AccountGet(args) => to_value(account_get(args)),
            RpcCommand::AccountHistory(args) => to_value(self.account_history(args)?),
            RpcCommand::AccountInfo(args) => to_value(self.account_info(args)?),
            RpcCommand::AccountKey(args) => to_value(account_key(args)),
            RpcCommand::AccountList(args) => to_value(self.account_list(args)?),
            RpcCommand::AccountMove(args) => to_value(self.account_move(args)?),
            RpcCommand::AccountsReceivable(args) => to_value(self.accounts_receivable(args)),
            RpcCommand::AccountRemove(args) => to_value(self.account_remove(args)?),
            RpcCommand::AccountRepresentative(args) => to_value(self.account_representative(args)?),
            RpcCommand::AccountWeight(args) => to_value(self.account_weight(args)),
            RpcCommand::AccountsRepresentatives(args) => {
                to_value(self.accounts_representatives(args))
            }
            RpcCommand::AccountsCreate(args) => to_value(self.accounts_create(args)?),
            RpcCommand::AccountsFrontiers(args) => to_value(self.accounts_frontiers(args)),
            RpcCommand::AvailableSupply => to_value(self.available_supply()),
            RpcCommand::BlockInfo(args) => to_value(self.block_info(args)?),
            RpcCommand::Blocks(args) => to_value(self.blocks(args)?),
            RpcCommand::BlockConfirm(args) => to_value(self.block_confirm(args)?),
            RpcCommand::BlockAccount(args) => to_value(self.block_account(args)?),
            RpcCommand::BlockCount => to_value(self.block_count()),
            RpcCommand::Receive(args) => to_value(self.receive(args)?),
            RpcCommand::BlockCreate(args) => to_value(self.block_create(args)?),
            RpcCommand::BlockHash(args) => to_value(block_hash(args)),
            RpcCommand::Bootstrap(args) => to_value(self.bootstrap(args)?),
            RpcCommand::BootstrapAny(args) => to_value(self.bootstrap_any(args)?),
            RpcCommand::BoostrapLazy(args) => to_value(self.bootstrap_lazy(args)?),
            RpcCommand::ConfirmationActive(args) => to_value(self.confirmation_active(args)),
            RpcCommand::ConfirmationInfo(args) => to_value(self.confirmation_info(args)?),
            RpcCommand::ConfirmationQuorum(args) => to_value(self.confirmation_quorum(args)),
            RpcCommand::Delegators(args) => to_value(self.delegators(args)),
            RpcCommand::DelegatorsCount(args) => to_value(self.delegators_count(args)),
            RpcCommand::DeterministicKey(args) => to_value(deterministic_key(args)),
            RpcCommand::Frontiers(args) => to_value(self.frontiers(args)),
            RpcCommand::FrontierCount => to_value(self.frontier_count()),
            RpcCommand::Keepalive(args) => to_value(self.keepalive(args)?),
            RpcCommand::KeyCreate => to_value(key_create()),
            RpcCommand::KeyExpand(args) => to_value(key_expand(args)?),
            RpcCommand::NodeId => to_value(self.node_id()),
            RpcCommand::PasswordChange(args) => to_value(self.password_change(args)?),
            RpcCommand::PasswordEnter(args) => to_value(self.password_enter(args)?),
            RpcCommand::Peers(args) => to_value(self.peers(args)),
            RpcCommand::ReceivableExists(args) => to_value(self.receivable_exists(args)?),
            RpcCommand::ReceiveMinimum => to_value(self.receive_minimum()),

            // Not implemented:
            RpcCommand::AccountRepresentativeSet(_) => self.not_implemented(),
            RpcCommand::AccountBalances(_) => self.not_implemented(),
            RpcCommand::WorkPeers => to_value(self.work_peers()),
            RpcCommand::WorkPeerAdd(args) => to_value(self.work_peer_add(args)),
            RpcCommand::WorkPeersClear => to_value(self.work_peers_clear()),
            RpcCommand::ConfirmationHistory(_) => self.not_implemented(),
            RpcCommand::DatabaseTxnTracker(_) => self.not_implemented(),
            RpcCommand::BootstrapLazy(_) => self.not_implemented(),
            RpcCommand::ReceiveMinimumSet(_) => self.not_implemented(),
            RpcCommand::Stats(_) => self.not_implemented(),
            RpcCommand::ActiveDifficulty => self.not_implemented(),

            // Not reviewed yet:
            RpcCommand::Ledger(args) => to_value(self.ledger(args)),
            RpcCommand::Receivable(args) => to_value(self.receivable(args)),
            RpcCommand::WalletCreate(args) => to_value(self.wallet_create(args)),
            RpcCommand::WalletAdd(args) => to_value(self.wallet_add(args)?),
            RpcCommand::WalletContains(args) => to_value(self.wallet_contains(args)?),
            RpcCommand::WalletDestroy(args) => to_value(self.wallet_destroy(args)),
            RpcCommand::WalletLock(args) => to_value(self.wallet_lock(args)?),
            RpcCommand::WalletLocked(args) => to_value(self.wallet_locked(args)?),
            RpcCommand::Stop => to_value(self.stop()),
            RpcCommand::Uptime => to_value(self.uptime()),
            RpcCommand::ValidateAccountNumber(_args) => {
                to_value(validate_account_number("TODO".to_string()))
            }
            RpcCommand::NanoToRaw(args) => to_value(nano_to_raw(args)),
            RpcCommand::RawToNano(args) => to_value(raw_to_nano(args)),
            RpcCommand::WalletAddWatch(args) => to_value(self.wallet_add_watch(args)?),
            RpcCommand::WalletRepresentative(args) => to_value(self.wallet_representative(args)?),
            RpcCommand::WorkSet(args) => to_value(self.work_set(args)?),
            RpcCommand::WorkGet(args) => to_value(self.work_get(args)?),
            RpcCommand::WalletWorkGet(args) => to_value(self.wallet_work_get(args)?),
            RpcCommand::WalletFrontiers(args) => to_value(self.wallet_frontiers(args)?),
            RpcCommand::WalletInfo(args) => to_value(self.wallet_info(args)?),
            RpcCommand::WalletExport(args) => to_value(wallet_export(args)),
            RpcCommand::PasswordValid(args) => to_value(self.password_valid(args)?),
            RpcCommand::PopulateBacklog => to_value(self.populate_backlog()),
            RpcCommand::Representatives(args) => to_value(self.representatives(args)),
            RpcCommand::StatsClear => to_value(self.stats_clear()),
            RpcCommand::UncheckedClear => to_value(self.unchecked_clear()),
            RpcCommand::Unopened(args) => to_value(self.unopened(args)),
            RpcCommand::Send(args) => to_value(self.send(args)),
            RpcCommand::SearchReceivableAll => to_value(self.search_receivable_all()),
            RpcCommand::WalletChangeSeed(args) => to_value(self.wallet_change_seed(args)),
            RpcCommand::AccountsBalances(args) => to_value(self.accounts_balances(args)),
            RpcCommand::BlocksInfo(args) => to_value(self.blocks_info(args)?),
            RpcCommand::Chain(args) => to_value(self.chain(args, false)),
            RpcCommand::Successors(args) => to_value(self.chain(args, true)),
            RpcCommand::WorkValidate(args) => to_value(self.work_validate(args)),
            RpcCommand::Sign(args) => to_value(self.sign(args)?),
            RpcCommand::Process(args) => to_value(self.process(args)?),
            RpcCommand::WorkCancel(args) => to_value(self.work_cancel(args)),
            RpcCommand::WalletReceivable(args) => to_value(self.wallet_receivable(args)?),
            RpcCommand::WalletRepresentativeSet(args) => {
                to_value(self.wallet_representative_set(args)?)
            }
            RpcCommand::SearchReceivable(args) => to_value(self.search_receivable(args)?),
            RpcCommand::WalletRepublish(args) => to_value(self.wallet_republish(args)?),
            RpcCommand::WalletBalances(args) => to_value(self.wallet_balances(args)),
            RpcCommand::WalletHistory(args) => to_value(self.wallet_history(args)?),
            RpcCommand::WalletLedger(args) => to_value(self.wallet_ledger(args)?),
            RpcCommand::RepresentativesOnline(args) => to_value(self.representatives_online(args)),
            RpcCommand::Unchecked(args) => to_value(self.unchecked(args)),
            RpcCommand::UncheckedGet(args) => to_value(self.unchecked_get(args)?),
            RpcCommand::UncheckedKeys(args) => to_value(self.unchecked_keys(args)),
            RpcCommand::WorkGenerate(args) => to_value(self.work_generate(args)?),
            RpcCommand::Republish(args) => to_value(self.republish(args)?),
            RpcCommand::Telemetry(args) => to_value(self.telemetry(args)?),
        }?;

        Ok(response)
    }

    fn check_control_enabled(&self, command: &RpcCommand) -> anyhow::Result<()> {
        if !self.enable_control && requires_control(command) {
            Err(anyhow!("RPC control is disabled"))
        } else {
            Ok(())
        }
    }

    fn load_block_any(&self, txn: &dyn Transaction, hash: &BlockHash) -> anyhow::Result<BlockEnum> {
        self.node
            .ledger
            .any()
            .get_block(txn, hash)
            .ok_or_else(|| anyhow!(Self::BLOCK_NOT_FOUND))
    }

    fn load_account(
        &self,
        txn: &dyn Transaction,
        account: &Account,
    ) -> anyhow::Result<AccountInfo> {
        self.node
            .ledger
            .any()
            .get_account(txn, account)
            .ok_or_else(|| anyhow!(Self::ACCOUNT_NOT_FOUND))
    }

    const BLOCK_NOT_FOUND: &str = "Block not found";
    const BLOCK_ERROR: &str = "Block error";
    const NOT_IMPLEMENTED: &str = "Not implemented yet";
    const ACCOUNT_NOT_FOUND: &str = "Account not found";
    const PEER_NOT_FOUND: &str = "Peer not found";

    fn not_implemented(&self) -> Result<Value, serde_json::Error> {
        Ok(Value::String(Self::NOT_IMPLEMENTED.to_string()))
    }
}

fn requires_control(command: &RpcCommand) -> bool {
    match command {
        RpcCommand::AccountCreate(_)
        | RpcCommand::AccountMove(_)
        | RpcCommand::AccountRemove(_)
        | RpcCommand::AccountRepresentativeSet(_)
        | RpcCommand::AccountsCreate(_)
        | RpcCommand::BlockCreate(_)
        | RpcCommand::BootstrapLazy(_)
        | RpcCommand::DatabaseTxnTracker(_)
        | RpcCommand::Keepalive(_)
        | RpcCommand::Ledger(_)
        | RpcCommand::NodeId
        | RpcCommand::PasswordChange(_)
        | RpcCommand::PopulateBacklog
        | RpcCommand::Receive(_)
        | RpcCommand::ReceiveMinimum
        | RpcCommand::ReceiveMinimumSet(_)
        | RpcCommand::SearchReceivable(_)
        | RpcCommand::SearchReceivableAll
        | RpcCommand::Send(_)
        | RpcCommand::Stop
        | RpcCommand::UncheckedClear
        | RpcCommand::Unopened(_)
        | RpcCommand::WalletAdd(_)
        | RpcCommand::WalletAddWatch(_)
        | RpcCommand::WalletChangeSeed(_)
        | RpcCommand::WalletCreate(_)
        | RpcCommand::WalletDestroy(_)
        | RpcCommand::WalletLock(_)
        | RpcCommand::WalletLedger(_)
        | RpcCommand::WalletRepresentativeSet(_)
        | RpcCommand::WalletReceivable(_)
        | RpcCommand::WalletRepublish(_)
        | RpcCommand::WalletWorkGet(_)
        | RpcCommand::WorkGenerate(_)
        | RpcCommand::WorkCancel(_)
        | RpcCommand::WorkGet(_)
        | RpcCommand::WorkSet(_)
        | RpcCommand::WorkPeerAdd(_)
        | RpcCommand::WorkPeers
        | RpcCommand::WorkPeersClear => true,
        RpcCommand::Stats(s) => match s.stats_type {
            StatsType::Objects => true,
            _ => false,
        },
        RpcCommand::Process(args) => args.force == Some(true),
        _ => false,
    }
}
