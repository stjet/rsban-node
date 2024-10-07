use super::account_get;
use super::block_confirm;
use super::keepalive;
use super::nano_to_raw;
use super::password_enter;
use super::stop;
use super::wallet_contains;
use super::wallet_destroy;
use super::wallet_frontiers;
use super::wallet_info;
use super::wallet_lock;
use super::wallet_locked;
use super::work_get;
use super::{
    account_create, account_list, account_move, account_remove, accounts_create, key_create,
    wallet_create,
};
use crate::account_balance;
use crate::uptime;
use anyhow::{Context, Result};
use axum::response::Response;
use axum::{extract::State, response::IntoResponse, routing::post, Json};
use axum::{
    http::{Request, StatusCode},
    middleware::map_request,
    Router,
};
use rsnano_node::node::Node;
use rsnano_rpc_messages::{AccountMoveArgs, RpcCommand, WalletAddArgs};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::info;
use super::peers;
use super::accounts_representatives;
use super::stats_clear;

use super::wallet_add;

use super::account_block_count;

use super::account_key;

use super::account_representative;

use super::account_weight;

use super::available_supply;

use super::block_account;

use super::block_count;

use super::frontier_count;

use super::validate_account_number;

use super::raw_to_nano;

use super::wallet_add_watch;

use super::wallet_representative;

use super::work_set;

use super::wallet_work_get;

use super::accounts_frontiers;

use super::frontiers;

use super::wallet_export;

use super::password_change;

use super::password_valid;

use super::deterministic_key;

use super::key_expand;

use super::populate_backlog;

use super::representatives;

use super::unchecked_clear;

use super::unopened;

use super::node_id;

use super::send;

#[derive(Clone)]
struct RpcService {
    node: Arc<Node>,
    enable_control: bool,
}

pub async fn run_rpc_server(
    node: Arc<Node>,
    server_addr: SocketAddr,
    enable_control: bool,
) -> Result<()> {
    let rpc_service = RpcService {
        node,
        enable_control,
    };

    let app = Router::new()
        .route("/", post(handle_rpc))
        .layer(map_request(set_header))
        .with_state(rpc_service);

    let listener = TcpListener::bind(server_addr)
        .await
        .context("Failed to bind to address")?;

    axum::serve(listener, app)
        .await
        .context("Failed to run the server")?;

    info!("RPC listening address: {}", server_addr);

    Ok(())
}

async fn handle_rpc(
    State(rpc_service): State<RpcService>,
    Json(rpc_command): Json<RpcCommand>,
) -> Response {
    let response = match rpc_command {
        RpcCommand::AccountCreate(args) => {
            account_create(
                rpc_service.node,
                rpc_service.enable_control,
                args.wallet,
                args.index,
                args.work,
            )
            .await
        }
        RpcCommand::AccountBalance(args) => {
            account_balance(rpc_service.node, args.account, args.include_only_confirmed).await
        }
        RpcCommand::AccountsCreate(args) => {
            accounts_create(rpc_service.node, rpc_service.enable_control, args).await
        }
        RpcCommand::AccountRemove(args) => {
            account_remove(
                rpc_service.node,
                rpc_service.enable_control,
                args.wallet,
                args.account,
            )
            .await
        }
        RpcCommand::AccountMove(AccountMoveArgs {
            wallet,
            source,
            accounts,
        }) => {
            account_move(
                rpc_service.node,
                rpc_service.enable_control,
                wallet,
                source,
                accounts,
            )
            .await
        }
        RpcCommand::AccountList(wallet_rpc_message) => {
            account_list(rpc_service.node, wallet_rpc_message.wallet).await
        }
        RpcCommand::WalletCreate(args) => {
            wallet_create(rpc_service.node, rpc_service.enable_control, args.seed).await
        }
        RpcCommand::KeyCreate => key_create().await,
        RpcCommand::WalletAdd(WalletAddArgs { wallet, key, work }) => {
            wallet_add(
                rpc_service.node,
                rpc_service.enable_control,
                wallet,
                key,
                work,
            )
            .await
        }
        RpcCommand::WalletContains(args) => {
            wallet_contains(rpc_service.node, args.wallet, args.account).await
        }
        RpcCommand::WalletDestroy(wallet_rpc_message) => {
            wallet_destroy(
                rpc_service.node,
                rpc_service.enable_control,
                wallet_rpc_message.wallet,
            )
            .await
        }
        RpcCommand::WalletLock(wallet_rpc_message) => {
            wallet_lock(
                rpc_service.node,
                rpc_service.enable_control,
                wallet_rpc_message.wallet,
            )
            .await
        }
        RpcCommand::WalletLocked(wallet_message_rpc) => {
            wallet_locked(rpc_service.node, wallet_message_rpc.wallet).await
        }
        RpcCommand::Stop => stop(rpc_service.node, rpc_service.enable_control).await,
        RpcCommand::AccountBlockCount(account_rpc_message) => {
            account_block_count(rpc_service.node, account_rpc_message.value).await
        }
        RpcCommand::AccountKey(account_rpc_message) => account_key(account_rpc_message.value).await,
        RpcCommand::AccountGet(args) => account_get(args.key).await,
        RpcCommand::AccountRepresentative(account_rpc_message) => {
            account_representative(rpc_service.node, account_rpc_message.value).await
        }
        RpcCommand::AccountWeight(account_rpc_message) => {
            account_weight(rpc_service.node, account_rpc_message.value).await
        }
        RpcCommand::AvailableSupply => available_supply(rpc_service.node).await,
        RpcCommand::BlockConfirm(block_hash_rpc_message) => {
            block_confirm(rpc_service.node, block_hash_rpc_message.value).await
        }
        RpcCommand::BlockCount => block_count(rpc_service.node).await,
        RpcCommand::BlockAccount(msg) => block_account(rpc_service.node, msg.value).await,
        RpcCommand::Uptime => uptime(rpc_service.node).await,
        RpcCommand::Keepalive(arg) => {
            keepalive(
                rpc_service.node,
                rpc_service.enable_control,
                arg.address,
                arg.port,
            )
            .await
        }
        RpcCommand::FrontierCount => frontier_count(rpc_service.node).await,
        RpcCommand::ValidateAccountNumber(_) => validate_account_number().await,
        RpcCommand::NanoToRaw(amount_rpc_message) => nano_to_raw(amount_rpc_message.value).await,
        RpcCommand::RawToNano(amount_rpc_message) => raw_to_nano(amount_rpc_message.value).await,
        RpcCommand::WalletAddWatch(args) => {
            wallet_add_watch(
                rpc_service.node,
                rpc_service.enable_control,
                args.wallet,
                args.accounts,
            )
            .await
        }
        RpcCommand::WalletRepresentative(args) => {
            wallet_representative(rpc_service.node, args.wallet).await
        }
        RpcCommand::WorkSet(args) => {
            work_set(
                rpc_service.node,
                rpc_service.enable_control,
                args.wallet,
                args.account,
                args.work,
            )
            .await
        }
        RpcCommand::WorkGet(args) => {
            work_get(
                rpc_service.node,
                rpc_service.enable_control,
                args.wallet,
                args.account,
            )
            .await
        }
        RpcCommand::WalletWorkGet(args) => {
            wallet_work_get(rpc_service.node, rpc_service.enable_control, args.wallet).await
        }
        RpcCommand::AccountsFrontiers(args) => {
            accounts_frontiers(rpc_service.node, args.accounts).await
        }
        RpcCommand::WalletFrontiers(args) => wallet_frontiers(rpc_service.node, args.wallet).await,
        RpcCommand::Frontiers(args) => frontiers(rpc_service.node, args.account, args.count).await,
        RpcCommand::WalletInfo(args) => wallet_info(rpc_service.node, args.wallet).await,
        RpcCommand::WalletExport(args) => wallet_export(args.wallet).await,
        RpcCommand::PasswordChange(args) => {
            password_change(
                rpc_service.node,
                rpc_service.enable_control,
                args.wallet,
                args.password,
            )
            .await
        }
        RpcCommand::PasswordEnter(args) => {
            password_enter(rpc_service.node, args.wallet, args.password).await
        }
        RpcCommand::PasswordValid(args) => password_valid(rpc_service.node, args.wallet).await,
        RpcCommand::DeterministicKey(args) => deterministic_key(args.seed, args.index).await,
        RpcCommand::KeyExpand(args) => key_expand(args.key).await,
        RpcCommand::Peers(args) => peers(rpc_service.node, args.peer_details).await,
        RpcCommand::PopulateBacklog => populate_backlog(rpc_service.node).await,
        RpcCommand::Representatives(args) => representatives(rpc_service.node, args.count, args.sorting).await,
        RpcCommand::AccountsRepresentatives(args) => accounts_representatives(rpc_service.node, args.accounts).await,
        RpcCommand::StatsClear => stats_clear(rpc_service.node).await,
        RpcCommand::UncheckedClear => unchecked_clear(rpc_service.node).await,
        RpcCommand::Unopened(args) => unopened(rpc_service.node, rpc_service.enable_control, args.account, args.count, args.threshold).await,
        RpcCommand::NodeId => node_id(rpc_service.node, rpc_service.enable_control).await,
        RpcCommand::Send(args) => send(rpc_service.node, rpc_service.enable_control, args).await,
        _ => todo!(),
    };

    (StatusCode::OK, response).into_response()
}

async fn set_header<B>(mut request: Request<B>) -> Request<B> {
    request
        .headers_mut()
        .insert("Content-Type", "application/json".parse().unwrap());
    request
}
