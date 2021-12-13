mod program_args;
mod rpc_client;
mod block_factory;
pub use block_factory::*;
use anyhow::anyhow;
use anyhow::Result;
use program_args::*;
use reqwest::Url;
pub use rpc_client::*;
use rsnano::secure::DEV_GENESIS_KEY;
use rsnano::secure::DEV_NETWORK_PARAMS;
use std::path::Path;
use std::process::Child;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tokio::time::Instant;

use rsnano::{
    config::{
        force_nano_dev_network, get_node_toml_config_path, get_rpc_toml_config_path, DaemonConfig,
        NetworkConstants, RpcConfig,
    },
    secure::{unique_path, NetworkParams},
    utils::TomlConfig,
};

const RPC_PORT_START: u16 = 60000;
const PEERING_PORT_START: u16 = 61000;
const IPC_PORT_START: u16 = 62000;

fn write_config_files(data_path: &Path, index: usize) -> Result<()> {
    let network_params = NetworkParams::new(NetworkConstants::active_network())?;
    write_node_config(index, data_path, &network_params)?;
    write_rpc_config(index, data_path, &network_params)?;
    Ok(())
}

fn write_node_config(index: usize, data_path: &Path, network_params: &NetworkParams) -> Result<()> {
    let mut daemon_config = DaemonConfig::new(&network_params)?;
    daemon_config.node.peering_port = PEERING_PORT_START + index as u16;
    daemon_config
        .node
        .ipc_config
        .transport_tcp
        .transport
        .enabled = true;
    daemon_config.node.ipc_config.transport_tcp.port = IPC_PORT_START + index as u16;
    daemon_config.node.use_memory_pools = (index % 2) == 0;
    let mut toml = TomlConfig::new();
    daemon_config.serialize_toml(&mut toml)?;
    toml.write(get_node_toml_config_path(data_path))?;
    Ok(())
}

fn write_rpc_config(index: usize, data_path: &Path, network_params: &NetworkParams) -> Result<()> {
    let mut rpc_config = RpcConfig::new(&network_params.network);
    rpc_config.port = RPC_PORT_START + index as u16;
    rpc_config.enable_control = true;
    rpc_config.rpc_process.ipc_port = IPC_PORT_START + index as u16;
    let mut toml_rpc = TomlConfig::new();
    rpc_config.serialize_toml(&mut toml_rpc)?;
    toml_rpc.write(get_rpc_toml_config_path(data_path))?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    force_nano_dev_network();
    let args = ProgramArgs::parse()?;
    args.validate_paths()?;

    let mut data_paths = Vec::new();
    for i in 0..args.node_count {
        let data_path = unique_path().ok_or_else(|| anyhow!("no unique path"))?;
        std::fs::create_dir(data_path.as_path())?;
        write_config_files(data_path.as_path(), i)?;
        data_paths.push(data_path);
    }

    let current_network = DEV_NETWORK_PARAMS.network.get_current_network_as_string();
    let mut nodes: Vec<Child> = Vec::new();
    let mut rpc_servers: Vec<Child> = Vec::new();
    for data_path in &data_paths {
        nodes.push(spawn_nano_node(&args.node_path, data_path, current_network));
        rpc_servers.push(spawn_nano_rpc(&args.rpc_path, data_path, current_network));
    }

    println!("Waiting for nodes to spin up...");
    sleep(Duration::from_secs(7)).await;
    println!("Connecting nodes...");

    let primary_node_client = Arc::new(RpcClient::new(Url::parse(&format!(
        "http://[::1]:{}/",
        RPC_PORT_START
    ))?));

    for i in 0..args.node_count {
        primary_node_client
            .keepalive_rpc(PEERING_PORT_START + i as u16)
            .await?;
    }

    println!("Beginning tests");

    // Create keys
    let mut destination_accounts = Vec::new();
    for _ in 0..args.destination_count {
        let acc = primary_node_client.key_create_rpc().await?;
        destination_accounts.push(acc);
    }
    let destination_accounts = Arc::new(destination_accounts);

    // Create wallet
    let wallet = Arc::new(primary_node_client.wallet_create_rpc().await?);

    // Add genesis account to it
    primary_node_client
        .wallet_add_rpc(&wallet, &DEV_GENESIS_KEY.private_key().encode_hex())
        .await?;

    // Add destination accounts
    for account in destination_accounts.iter() {
        primary_node_client
            .wallet_add_rpc(&wallet, &account.private_key)
            .await?;
    }

    let known_account_info = create_send_and_receive_blocks(
        args.send_count,
        args.simultaneous_process_calls,
        destination_accounts.clone(),
        wallet.clone(),
        primary_node_client.clone(),
    )
    .await?;

    println!("Waiting for nodes to catch up...");
    let timer = Instant::now();

    for i in 1..args.node_count {
        let node_url = format!("http://[::1]:{}/", RPC_PORT_START + i as u16);
        let node_client = RpcClient::new(Url::parse(&node_url)?);
        println!("starting check for {}", node_url);
        for (acc, info) in &known_account_info {
            loop {
                if let Ok(other_account_info) = node_client.account_info_rpc(acc).await {
                    if info == &other_account_info {
                        println!("OK node {}", node_url);
                        // Found the account in this node
                        break;
                    }
                }

                if timer.elapsed() > Duration::from_secs(120) {
                    panic!("Timed out");
                }

                sleep(Duration::from_secs(1)).await;
            }
        }

        node_client.stop_rpc().await?;
    }

    println!("catching up took {:?}", timer.elapsed());

    // Stop main node
    primary_node_client.stop_rpc().await?;

    for mut node in nodes {
        node.wait()?;
    }

    for mut rpc_server in rpc_servers {
        rpc_server.wait()?;
    }

    println!("Done!");
    Ok(())
}

fn spawn_nano_rpc(rpc_path: &Path, data_path: &Path, network: &str) -> Child {
    Command::new(rpc_path.as_os_str())
        .arg("--daemon")
        .arg("--data_path")
        .arg(data_path)
        .arg("--network")
        .arg(network)
        .spawn()
        .expect("could not spawn rpc server")
}

fn spawn_nano_node(node_path: &Path, data_path: &Path, network: &str) -> Child {
    Command::new(node_path.as_os_str())
        .arg("--daemon")
        .arg("--data_path")
        .arg(data_path)
        .arg("--network")
        .arg(network)
        .spawn()
        .expect("could not spawn node")
}
