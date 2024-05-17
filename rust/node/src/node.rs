use crate::{config::NodeConfig, stats::Stats, utils::AsyncRuntime, NetworkParams};
use rsnano_core::KeyPair;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tracing::info;

pub struct Node {
    pub async_rt: Arc<AsyncRuntime>,
    application_path: PathBuf,
    pub node_id: KeyPair,
    pub config: NodeConfig,
    network_params: NetworkParams,
    pub stats: Arc<Stats>,
}

impl Node {
    pub fn new(
        async_rt: Arc<AsyncRuntime>,
        application_path: impl Into<PathBuf>,
        config: NodeConfig,
        network_params: NetworkParams,
    ) -> Self {
        let application_path = application_path.into();
        let node_id = load_or_create_node_id(&application_path);
        Self {
            async_rt,
            application_path,
            node_id,
            network_params,
            stats: Arc::new(Stats::new(config.stat_config.clone())),
            config,
        }
    }
}

fn load_or_create_node_id(path: &Path) -> KeyPair {
    let mut private_key_path = PathBuf::from(path);
    private_key_path.push("node_id_private.key");
    if private_key_path.exists() {
        info!("Reading node id from: '{:?}'", private_key_path);
        let content =
            std::fs::read_to_string(&private_key_path).expect("Could not read node id file");
        KeyPair::from_priv_key_hex(&content).expect("Could not read node id")
    } else {
        std::fs::create_dir_all(path).expect("Could not create app dir");
        info!("Generating a new node id, saving to: '{:?}'", path);
        let keypair = KeyPair::new();
        std::fs::write(
            private_key_path,
            keypair.private_key().encode_hex().as_bytes(),
        )
        .expect("Could not write node id file");
        keypair
    }
}
