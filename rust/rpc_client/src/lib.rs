use crate::AccountBalanceResponse;
use anyhow::{anyhow, Ok, Result};
use reqwest::Client;
pub use reqwest::Url;
use rsnano_core::{
    Account, Amount, BlockHash, HashOrAccount, JsonBlock, PublicKey, RawKey, WalletId, WorkNonce,
};
use rsnano_rpc_messages::*;
use serde::Serialize;
use serde_json::{from_value, Value};
use std::time::Duration;

pub struct NanoRpcClient {
    url: Url,
    client: Client,
}

impl NanoRpcClient {
    pub fn new(url: Url) -> Self {
        Self {
            url,
            client: reqwest::ClientBuilder::new()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap(),
        }
    }

    pub async fn telemetry(&self, args: TelemetryArgs) -> Result<TelemetryResponose> {
        let cmd = RpcCommand::telemetry(args);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn account_get(&self, key: PublicKey) -> Result<AccountResponse> {
        let cmd = RpcCommand::account_get(key);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn work_peers(&self) -> Result<WorkPeersDto> {
        let cmd = RpcCommand::work_peers();
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn work_peer_add(&self, args: AddressWithPortArgs) -> Result<SuccessResponse> {
        let cmd = RpcCommand::work_peer_add(args);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn work_peers_clear(&self) -> Result<SuccessResponse> {
        let cmd = RpcCommand::work_peers_clear();
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn block_create(
        &self,
        block_create_args: BlockCreateArgs,
    ) -> Result<BlockCreateResponse> {
        let cmd = RpcCommand::block_create(block_create_args);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn republish(&self, args: impl Into<RepublishArgs>) -> Result<BlockHashesResponse> {
        let cmd = RpcCommand::republish(args.into());
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn work_generate(
        &self,
        args: impl Into<WorkGenerateArgs>,
    ) -> Result<WorkGenerateDto> {
        let cmd = RpcCommand::work_generate(args.into());
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn ledger(&self, ledger_args: LedgerArgs) -> Result<LedgerResponse> {
        let cmd = RpcCommand::ledger(ledger_args);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn confirmation_info(
        &self,
        args: impl Into<ConfirmationInfoArgs>,
    ) -> Result<ConfirmationInfoDto> {
        let cmd = RpcCommand::confirmation_info(args.into());
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn unchecked_keys(
        &self,
        key: HashOrAccount,
        count: Option<u64>,
    ) -> Result<UncheckedKeysResponse> {
        let cmd = RpcCommand::unchecked_keys(key, count);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn unchecked_get(&self, hash: BlockHash) -> Result<UncheckedGetResponse> {
        let cmd = RpcCommand::unchecked_get(hash);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn unchecked(&self, count: u64) -> Result<UncheckedResponse> {
        let cmd = RpcCommand::unchecked(count);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn representatives_online(
        &self,
        args: RepresentativesOnlineArgs,
    ) -> Result<RepresentativesOnlineResponse> {
        let detailed = args.weight.unwrap_or(false);
        let cmd = RpcCommand::representatives_online(args);
        let result = self.rpc_request(&cmd).await?;
        if detailed {
            let detailed: DetailedRepresentativesOnline = serde_json::from_value(result)?;
            Ok(RepresentativesOnlineResponse::Detailed(detailed))
        } else {
            let simple: SimpleRepresentativesOnline = serde_json::from_value(result)?;
            Ok(RepresentativesOnlineResponse::Simple(simple))
        }
    }

    pub async fn receivable_exists(
        &self,
        args: impl Into<ReceivableExistsArgs>,
    ) -> Result<ExistsResponse> {
        let cmd = RpcCommand::receivable_exists(args.into());
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn receivable(&self, args: impl Into<ReceivableArgs>) -> Result<ReceivableResponse> {
        let args = args.into();
        let source: bool = args.source.unwrap_or_default().into();
        let min_version: bool = args.min_version.unwrap_or_default().into();
        let sort: bool = args.sorting.unwrap_or_default().into();
        let simple =
            args.threshold.unwrap_or_default().is_zero() && !source && !min_version && !sort;

        let cmd = RpcCommand::Receivable(args);
        let result = self.rpc_request(&cmd).await?;
        if simple {
            let blocks = serde_json::from_value::<ReceivableSimple>(result)?;
            Ok(ReceivableResponse::Simple(blocks))
        } else if source || min_version {
            let blocks = serde_json::from_value::<ReceivableSource>(result)?;
            Ok(ReceivableResponse::Source(blocks))
        } else {
            let blocks = serde_json::from_value::<ReceivableThreshold>(result)?;
            Ok(ReceivableResponse::Threshold(blocks))
        }
    }

    pub async fn accounts_receivable(
        &self,
        args: impl Into<AccountsReceivableArgs>,
    ) -> Result<AccountsReceivableResponse> {
        let args = args.into();

        let threshold = args.threshold.unwrap_or_default();
        let source = unwrap_bool_or_false(args.source);
        let simple = threshold.is_zero() && !source && !unwrap_bool_or_false(args.sorting);
        let cmd = RpcCommand::AccountsReceivable(args);
        let result = self.rpc_request(&cmd).await?;
        if simple {
            Ok(AccountsReceivableResponse::Simple(serde_json::from_value(
                result,
            )?))
        } else if source {
            Ok(AccountsReceivableResponse::Source(serde_json::from_value(
                result,
            )?))
        } else {
            Ok(AccountsReceivableResponse::Threshold(
                serde_json::from_value(result)?,
            ))
        }
    }

    pub async fn wallet_ledger(
        &self,
        args: impl Into<WalletLedgerArgs>,
    ) -> Result<WalletLedgerResponse> {
        let cmd = RpcCommand::wallet_ledger(args.into());
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn wallet_history(
        &self,
        args: impl Into<WalletHistoryArgs>,
    ) -> Result<WalletHistoryResponse> {
        let cmd = RpcCommand::wallet_history(args.into());
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn wallet_republish(
        &self,
        wallet: WalletId,
        count: u64,
    ) -> Result<BlockHashesResponse> {
        let cmd = RpcCommand::wallet_republish(wallet, count);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn search_receivable(&self, wallet: WalletId) -> Result<StartedResponse> {
        let cmd = RpcCommand::search_receivable(wallet);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn wallet_representative_set(
        &self,
        args: WalletRepresentativeSetArgs,
    ) -> Result<SetResponse> {
        let cmd = RpcCommand::wallet_representative_set(args);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn wallet_receivable(
        &self,
        args: WalletReceivableArgs,
    ) -> Result<AccountsReceivableResponse> {
        let cmd = RpcCommand::WalletReceivable(args);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn bootstrap_lazy(
        &self,
        args: impl Into<BootstrapLazyArgs>,
    ) -> Result<BootstrapLazyResponse> {
        let cmd = RpcCommand::bootstrap_lazy(args.into());
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn bootstrap_any(&self, args: BootstrapAnyArgs) -> Result<SuccessResponse> {
        let cmd = RpcCommand::bootstrap_any(args);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn bootstrap(&self, args: BootstrapArgs) -> Result<SuccessResponse> {
        let cmd = RpcCommand::bootstrap(args);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn work_cancel(&self, hash: BlockHash) -> Result<SuccessResponse> {
        let cmd = RpcCommand::work_cancel(hash);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn process(&self, process_args: impl Into<ProcessArgs>) -> Result<HashRpcMessage> {
        let cmd = RpcCommand::process(process_args.into());
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn sign(&self, args: impl Into<SignArgs>) -> Result<SignResponse> {
        let cmd = RpcCommand::sign(args.into());
        let json = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(json)?)
    }

    pub async fn account_history(
        &self,
        args: impl Into<AccountHistoryArgs>,
    ) -> Result<AccountHistoryResponse> {
        let cmd = RpcCommand::account_history(args.into());
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn account_balance(
        &self,
        args: impl Into<AccountBalanceArgs>,
    ) -> Result<AccountBalanceResponse> {
        let cmd = RpcCommand::AccountBalance(args.into());
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn account_create(
        &self,
        args: impl Into<AccountCreateArgs>,
    ) -> Result<AccountResponse> {
        let cmd = RpcCommand::account_create(args.into());
        let result = self.rpc_request(&cmd).await?;
        Ok(from_value(result)?)
    }

    pub async fn accounts_create(
        &self,
        wallet: WalletId,
        count: u64,
    ) -> Result<AccountsRpcMessage> {
        self.accounts_create_args(AccountsCreateArgs::build(wallet, count).finish())
            .await
    }

    pub async fn accounts_create_args(
        &self,
        args: impl Into<AccountsCreateArgs>,
    ) -> Result<AccountsRpcMessage> {
        let cmd = RpcCommand::AccountsCreate(args.into());
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn account_remove(&self, wallet: WalletId, account: Account) -> Result<RemovedDto> {
        let cmd = RpcCommand::account_remove(wallet, account);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn account_move(
        &self,
        wallet: WalletId,
        source: WalletId,
        account: Vec<Account>,
    ) -> Result<MovedResponse> {
        let cmd = RpcCommand::account_move(wallet, source, account);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn account_list(&self, wallet: WalletId) -> Result<AccountsRpcMessage> {
        let cmd = RpcCommand::account_list(wallet);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn wallet_create(&self, seed: Option<RawKey>) -> Result<WalletCreateResponse> {
        let cmd = RpcCommand::wallet_create(seed);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn wallet_contains(
        &self,
        wallet: WalletId,
        account: Account,
    ) -> Result<ExistsResponse> {
        let cmd = RpcCommand::wallet_contains(wallet, account);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn wallet_destroy(&self, wallet: WalletId) -> Result<DestroyedResponse> {
        let cmd = RpcCommand::wallet_destroy(wallet);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn wallet_lock(&self, wallet: WalletId) -> Result<LockedResponse> {
        let cmd = RpcCommand::wallet_lock(wallet);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn wallet_locked(&self, wallet: WalletId) -> Result<LockedResponse> {
        let cmd = RpcCommand::wallet_locked(wallet);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn stop(&self) -> Result<SuccessResponse> {
        let cmd = RpcCommand::stop();
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn account_block_count(&self, account: Account) -> Result<AccountBlockCountResponse> {
        let cmd = RpcCommand::account_block_count(account);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn account_key(&self, account: Account) -> Result<KeyResponse> {
        let cmd = RpcCommand::account_key(account);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn account_representative(
        &self,
        account: Account,
    ) -> Result<AccountRepresentativeDto> {
        let cmd = RpcCommand::account_representative(account);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn account_weight(&self, account: Account) -> Result<WeightDto> {
        let cmd = RpcCommand::account_weight(account);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn available_supply(&self) -> Result<AvailableSupplyReponse> {
        let cmd = RpcCommand::AvailableSupply;
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn block_account(&self, hash: BlockHash) -> Result<AccountResponse> {
        let cmd = RpcCommand::block_account(hash);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn block_confirm(&self, hash: BlockHash) -> Result<StartedResponse> {
        let cmd = RpcCommand::block_confirm(hash);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn block_count(&self) -> Result<BlockCountResponse> {
        let cmd = RpcCommand::BlockCount;
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn uptime(&self) -> Result<UptimeResponse> {
        let cmd = RpcCommand::uptime();
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn frontier_count(&self) -> Result<CountResponse> {
        let cmd = RpcCommand::frontier_count();
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn validate_account_number(
        &self,
        account: impl Into<String>,
    ) -> Result<ValidResponse> {
        let cmd = RpcCommand::validate_account_number(account.into());
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn nano_to_raw(&self, amount: u64) -> Result<AmountRpcMessage> {
        let cmd = RpcCommand::nano_to_raw(amount);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn raw_to_nano(&self, amount: Amount) -> Result<AmountRpcMessage> {
        let cmd = RpcCommand::raw_to_nano(amount);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn wallet_add_watch(
        &self,
        wallet: WalletId,
        accounts: Vec<Account>,
    ) -> Result<SuccessResponse> {
        let cmd = RpcCommand::wallet_add_watch(wallet, accounts);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn wallet_representative(
        &self,
        wallet: WalletId,
    ) -> Result<WalletRepresentativeResponse> {
        let cmd = RpcCommand::wallet_representative(wallet);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn work_set(
        &self,
        wallet: WalletId,
        account: Account,
        work: WorkNonce,
    ) -> Result<SuccessResponse> {
        let cmd = RpcCommand::work_set(wallet, account, work);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn work_get(&self, wallet: WalletId, account: Account) -> Result<WorkResponse> {
        let cmd = RpcCommand::work_get(wallet, account);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn wallet_work_get(&self, wallet: WalletId) -> Result<AccountsWithWorkResponse> {
        let cmd = RpcCommand::wallet_work_get(wallet);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn accounts_frontiers(&self, accounts: Vec<Account>) -> Result<FrontiersResponse> {
        let cmd = RpcCommand::accounts_frontiers(accounts);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn wallet_frontiers(&self, wallet: WalletId) -> Result<FrontiersResponse> {
        let cmd = RpcCommand::wallet_frontiers(wallet);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn frontiers(&self, account: Account, count: u64) -> Result<FrontiersResponse> {
        let cmd = RpcCommand::frontiers(account, count);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn wallet_info(&self, wallet: WalletId) -> Result<WalletInfoResponse> {
        let cmd = RpcCommand::wallet_info(wallet);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn wallet_export(&self, wallet: WalletId) -> Result<JsonResponse> {
        let cmd = RpcCommand::wallet_export(wallet);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn password_change(
        &self,
        wallet: WalletId,
        password: String,
    ) -> Result<ChangedResponse> {
        let cmd = RpcCommand::password_change(wallet, password);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn password_enter(
        &self,
        wallet: WalletId,
        password: String,
    ) -> Result<ValidResponse> {
        let cmd = RpcCommand::password_enter(wallet, password);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn password_valid(&self, wallet: WalletId) -> Result<ValidResponse> {
        let cmd = RpcCommand::password_valid(wallet);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn deterministic_key(&self, seed: RawKey, index: u32) -> Result<KeyPairDto> {
        let cmd = RpcCommand::deterministic_key(seed, index);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn key_expand(&self, key: RawKey) -> Result<KeyPairDto> {
        let cmd = RpcCommand::key_expand(key);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn peers(&self, peer_details: Option<bool>) -> Result<PeersDto> {
        let cmd = RpcCommand::peers(peer_details);
        let result = self.rpc_request(&cmd).await?;
        if peer_details.unwrap_or_default() {
            let peers: DetailedPeers = serde_json::from_value(result)?;
            Ok(PeersDto::Detailed(peers))
        } else {
            let peers: SimplePeers = serde_json::from_value(result)?;
            Ok(PeersDto::Simple(peers))
        }
    }

    pub async fn populate_backlog(&self) -> Result<SuccessResponse> {
        let cmd = RpcCommand::populate_backlog();
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn representatives(
        &self,
        count: Option<usize>,
        sorting: Option<bool>,
    ) -> Result<RepresentativesResponse> {
        let cmd = RpcCommand::representatives(count, sorting);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn accounts_representatives(
        &self,
        accounts: Vec<Account>,
    ) -> Result<AccountsRepresentativesResponse> {
        let cmd = RpcCommand::accounts_representatives(accounts);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn stats_clear(&self) -> Result<SuccessResponse> {
        let cmd = RpcCommand::stats_clear();
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn unchecked_clear(&self) -> Result<SuccessResponse> {
        let cmd = RpcCommand::unchecked_clear();
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn unopened(&self, args: impl Into<UnopenedArgs>) -> Result<AccountsWithAmountsDto> {
        let cmd = RpcCommand::unopened(args.into());
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn node_id(&self) -> Result<NodeIdResponse> {
        let cmd = RpcCommand::node_id();
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn search_receivable_all(&self) -> Result<SuccessResponse> {
        let cmd = RpcCommand::search_receivable_all();
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn receive_minimum(&self) -> Result<AmountRpcMessage> {
        let cmd = RpcCommand::receive_minimum();
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn wallet_change_seed(
        &self,
        args: impl Into<WalletChangeSeedArgs>,
    ) -> Result<WalletChangeSeedResponse> {
        let cmd = RpcCommand::wallet_change_seed(args.into());
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn delegators(&self, args: impl Into<DelegatorsArgs>) -> Result<DelegatorsResponse> {
        let cmd = RpcCommand::delegators(args.into());
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn delegators_count(&self, account: Account) -> Result<CountResponse> {
        let cmd = RpcCommand::delegators_count(account);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn block_hash(&self, block: JsonBlock) -> Result<HashRpcMessage> {
        let cmd = RpcCommand::block_hash(block);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn accounts_balances(
        &self,
        args: impl Into<AccountsBalancesArgs>,
    ) -> Result<AccountsBalancesResponse> {
        let cmd = RpcCommand::AccountsBalances(args.into());
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn wallet_balances(
        &self,
        args: impl Into<WalletBalancesArgs>,
    ) -> Result<AccountsBalancesResponse> {
        let cmd = RpcCommand::wallet_balances(args.into());
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn block_info(&self, hash: BlockHash) -> Result<BlockInfoResponse> {
        let cmd = RpcCommand::block_info(hash);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn blocks(&self, blocks: Vec<BlockHash>) -> Result<BlocksResponse> {
        let cmd = RpcCommand::blocks(blocks);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn blocks_info(&self, blocks: Vec<BlockHash>) -> Result<BlocksInfoResponse> {
        let cmd = RpcCommand::blocks_info(blocks);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn successors(&self, args: impl Into<ChainArgs>) -> Result<BlockHashesResponse> {
        let cmd = RpcCommand::successors(args.into());
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn chain(&self, args: ChainArgs) -> Result<BlockHashesResponse> {
        let cmd = RpcCommand::chain(args);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn confirmation_active(
        &self,
        announcements: Option<u64>,
    ) -> Result<ConfirmationActiveResponse> {
        let cmd = RpcCommand::confirmation_active(announcements);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn confirmation_quorum(
        &self,
        peer_details: Option<bool>,
    ) -> Result<ConfirmationQuorumResponse> {
        let cmd = RpcCommand::confirmation_quorum(peer_details);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn work_validate(
        &self,
        args: impl Into<WorkValidateArgs>,
    ) -> Result<WorkValidateResponse> {
        let cmd = RpcCommand::work_validate(args);
        let result = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn account_info(
        &self,
        args: impl Into<AccountInfoArgs>,
    ) -> Result<AccountInfoResponse> {
        let account_info_args = args.into();
        let cmd = RpcCommand::account_info(account_info_args);
        let result = self.rpc_request(&cmd).await?;
        Ok(from_value(result)?)
    }

    pub async fn receive(&self, args: ReceiveArgs) -> Result<BlockDto> {
        let request = RpcCommand::Receive(args);
        let result = self.rpc_request(&request).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn send(&self, args: SendArgs) -> Result<BlockDto> {
        let request = RpcCommand::send(args);
        let result = self.rpc_request(&request).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn send_receive(
        &self,
        wallet: WalletId,
        source: Account,
        destination: Account,
        amount: Amount,
    ) -> Result<()> {
        let send_args = SendArgs {
            wallet,
            source,
            destination,
            amount,
            ..Default::default()
        };
        let block = self.send(send_args).await?;
        let receive_args = ReceiveArgs::builder(wallet, destination, block.block).build();
        self.receive(receive_args).await?;
        Ok(())
    }

    pub async fn keepalive(
        &self,
        address: impl Into<String>,
        port: u16,
    ) -> Result<StartedResponse> {
        let cmd = RpcCommand::keepalive(address, port);
        let json = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(json)?)
    }

    pub async fn key_create(&self) -> Result<KeyPairDto> {
        let cmd = RpcCommand::KeyCreate;
        let json = self.rpc_request(&cmd).await?;
        Ok(from_value(json)?)
    }

    pub async fn wallet_add(&self, args: WalletAddArgs) -> Result<AccountResponse> {
        let cmd = RpcCommand::wallet_add(args);
        let json = self.rpc_request(&cmd).await?;
        Ok(serde_json::from_value(json)?)
    }

    pub async fn stop_rpc(&self) -> Result<()> {
        self.rpc_request(&RpcCommand::Stop).await?;
        Ok(())
    }

    pub async fn stats(&self, stats_type: StatsType) -> Result<serde_json::Value> {
        self.rpc_request(&RpcCommand::Stats(StatsArgs { stats_type }))
            .await
    }

    pub async fn version(&self) -> Result<VersionResponse> {
        let json = self.rpc_request(&RpcCommand::Version).await?;
        Ok(serde_json::from_value(json)?)
    }

    async fn rpc_request<T>(&self, request: &T) -> Result<serde_json::Value>
    where
        T: Serialize,
    {
        let result = self
            .client
            .post(self.url.clone())
            .json(request)
            .send()
            .await?
            .error_for_status()?
            .json::<Value>()
            .await?;

        check_error(&result).map_err(|e| anyhow!("node returned error: \"{}\"", e))?;
        Ok(result)
    }
}
