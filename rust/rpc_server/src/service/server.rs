use super::wallet_contains;
use super::{
    account_create, account_list, account_move, account_remove, accounts_create, key_create,
    wallet_create,
};
use crate::account_balance;
use super::wallet_destroy;
use super::wallet_lock;
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

use super::wallet_add;

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
