use crate::command_handler::RpcCommandHandler;
use anyhow::{Context, Result};
use axum::{
    extract::State,
    http::{Request, StatusCode},
    middleware::map_request,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use rsnano_node::Node;
use rsnano_rpc_messages::RpcCommand;
use serde_json::to_string_pretty;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::info;

pub async fn run_rpc_server(
    node: Arc<Node>,
    listener: TcpListener,
    enable_control: bool,
) -> Result<()> {
    let command_handler = RpcCommandHandler::new(node, enable_control);

    let app = Router::new()
        .route("/", post(handle_rpc))
        .layer(map_request(set_json_content))
        .with_state(command_handler);

    info!("RPC listening address: {}", listener.local_addr()?);

    axum::serve(listener, app)
        .await
        .context("Failed to run the server")
}

async fn handle_rpc(
    State(command_handler): State<RpcCommandHandler>,
    Json(command): Json<RpcCommand>,
) -> Response {
    let response = command_handler.handle(command).await;
    (StatusCode::OK, to_string_pretty(&response).unwrap()).into_response()
}

/// JSON is the default and the only accepted content type!
async fn set_json_content<B>(mut request: Request<B>) -> Request<B> {
    request
        .headers_mut()
        .insert("Content-Type", "application/json".parse().unwrap());
    request
}
