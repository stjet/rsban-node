use crate::cli::get_path;
use anyhow::{anyhow, Result};
use clap::{ArgGroup, Parser};
use rsnano_store_lmdb::{
    tests::{Fixture, TEST_PEER_A, TEST_PEER_B},
    LmdbEnv, LmdbPeerStore,
};
use std::sync::Arc;

#[derive(Parser)]
#[command(group = ArgGroup::new("input")
    .args(&["data_path", "network"]))]
pub(crate) struct PeersArgs {
    #[arg(long, group = "input")]
    data_path: Option<String>,
    #[arg(long, group = "input")]
    network: Option<String>,
    #[arg(long)]
    test: bool,
}

impl PeersArgs {
    pub(crate) fn peers(&self) -> Result<()> {
        if self.test {
            let fixture = Fixture::with_stored_data(vec![TEST_PEER_A, TEST_PEER_B]);

            let mut txn = fixture.env.tx_begin_read();

            for peer in fixture.store.iter(&mut txn) {
                println!("{:?}", peer);
            }
        } else {
            let path = get_path(&self.data_path, &self.network).join("data.ldb");

            let env = Arc::new(LmdbEnv::new(&path)?);

            let peer_store = LmdbPeerStore::new(env.clone())
                .map_err(|e| anyhow!("Error opening store: {:?}", e))?;

            let mut txn = env.tx_begin_read();

            for peer in peer_store.iter(&mut txn) {
                println!("{:?}", peer);
            }
        }

        Ok(())
    }
}

/*pub const TEST_PEER_A: SocketAddrV6 =
    SocketAddrV6::new(Ipv6Addr::new(1, 2, 3, 4, 5, 6, 7, 8), 1000, 0, 0);

pub const TEST_PEER_B: SocketAddrV6 =
    SocketAddrV6::new(Ipv6Addr::new(3, 3, 3, 3, 3, 3, 3, 3), 2000, 0, 0);

pub struct Fixture {
    env: Arc<LmdbEnv>,
    store: LmdbPeerStore,
}

impl Fixture {
    pub fn with_stored_data(entries: Vec<SocketAddrV6>) -> Self {
        let mut env = LmdbEnv::new_null_with().database("peers", LmdbDatabase::new_null(42));

        for entry in entries {
            env = env.entry(
                &EndpointBytes::from(entry),
                &TimeBytes::from(SystemTime::UNIX_EPOCH),
            );
        }

        Self::with_env(env.build().build())
    }

    fn with_env(env: LmdbEnv) -> Self {
        let env = Arc::new(env);
        Self {
            env: env.clone(),
            store: LmdbPeerStore::new(env).unwrap(),
        }
    }
    }*/

#[cfg(test)]
mod tests {
    use assert_cmd::Command;

    const PEERS_STR: &str = "([1:2:3:4:5:6:7:8]:1000, SystemTime { tv_sec: 0, tv_nsec: 0 })\n([3:3:3:3:3:3:3:3]:2000, SystemTime { tv_sec: 0, tv_nsec: 0 })\n";

    #[test]
    fn peers() {
        Command::cargo_bin("rsnano_node")
            .unwrap()
            .args(["ledger", "read", "peers", "--test"])
            .assert()
            .success()
            .stdout(PEERS_STR);
    }
}
