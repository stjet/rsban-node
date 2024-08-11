use crate::format_error_message;
use crate::request::RpcRequest;
use crate::response::account_balance;
use anyhow::{Context, Result};
use axum::response::Response;
use axum::{extract::State, response::IntoResponse, routing::post, Json};
use axum::{
    http::{Request, StatusCode},
    middleware::map_request,
    Router,
};
use rsnano_node::node::Node;
use std::net::SocketAddr;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use tokio::net::TcpListener;

pub async fn run_rpc_server(node: Arc<Node>) -> Result<()> {
    let app = Router::new()
        .route("/", post(handle_rpc))
        .layer(map_request(set_header))
        .with_state(node);

    let server_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 7076);

    let listener = TcpListener::bind(server_addr)
        .await
        .context("Failed to bind to address")?;

    axum::serve(listener, app)
        .await
        .context("Failed to run the server")?;

    Ok(())
}

async fn handle_rpc(
    State(node): State<Arc<Node>>,
    Json(rpc_request): Json<RpcRequest>,
) -> Response {
    let response = match rpc_request {
        RpcRequest::AccountBalance {
            account,
            only_confirmed,
        } => account_balance(node, account, only_confirmed).await,
        RpcRequest::UnknownCommand => format_error_message("Unknown command"),
    };

    (StatusCode::OK, response).into_response()
}

async fn set_header<B>(mut request: Request<B>) -> Request<B> {
    request
        .headers_mut()
        .insert("Content-Type", "application/json".parse().unwrap());
    request
}
