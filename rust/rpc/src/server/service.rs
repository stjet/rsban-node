use anyhow::{Context, Result};
use axum::response::Response;
use axum::{extract::State, response::IntoResponse, routing::post, Json};
use axum::{
    http::{Request, StatusCode},
    middleware::map_request,
    Router,
};
use rsnano_node::node::Node;
use serde_json::{json, to_string_pretty};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::info;

use super::request::{NodeRpcRequest, RpcRequest, WalletRpcRequest};
use super::response::{account_balance, account_create};

#[derive(Clone)]
struct Service {
    node: Arc<Node>,
    enable_control: bool,
}

pub async fn run_rpc_server(
    node: Arc<Node>,
    server_addr: SocketAddr,
    enable_control: bool,
) -> Result<()> {
    let service = Service {
        node,
        enable_control,
    };

    let app = Router::new()
        .route("/", post(handle_rpc))
        .layer(map_request(set_header))
        .with_state(service);

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
    State(service): State<Service>,
    Json(rpc_request): Json<RpcRequest>,
) -> Response {
    let response = match rpc_request {
        RpcRequest::Node(node_request) => match node_request {
            NodeRpcRequest::AccountBalance {
                account,
                only_confirmed,
            } => account_balance(service.node, account, only_confirmed).await,
        },
        RpcRequest::Wallet(wallet_request) => match wallet_request {
            WalletRpcRequest::AccountCreate { wallet, index } => {
                if service.enable_control {
                    account_create(service.node, wallet, index).await
                } else {
                    format_error_message("Enable control is disabled")
                }
            }
            WalletRpcRequest::UnknownCommand => format_error_message("Unknown command"),
        },
    };

    (StatusCode::OK, response).into_response()
}

async fn set_header<B>(mut request: Request<B>) -> Request<B> {
    request
        .headers_mut()
        .insert("Content-Type", "application/json".parse().unwrap());
    request
}

pub(crate) fn format_error_message(error: &str) -> String {
    let json_value = json!({ "error": error });
    to_string_pretty(&json_value).unwrap()
}
