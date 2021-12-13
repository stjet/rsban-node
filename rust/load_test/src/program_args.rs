use anyhow::{bail, Result};
use clap::{App, Arg};
use std::path::{Path, PathBuf};

pub struct ProgramArgs {
    pub node_count: usize,
    pub destination_count: usize,
    pub send_count: usize,
    pub simultaneous_process_calls: usize,
    pub node_path: PathBuf,
    pub rpc_path: PathBuf,
}

impl ProgramArgs {
    pub fn parse() -> Result<Self> {
        let matches = App::new("Nano Load Test")
        .about("This launches a node and fires a lot of send/recieve RPC requests at it (configurable), then other nodes are tested to make sure they observe these blocks as well.")
        .arg(Arg::with_name("node_count").short("n").long("node_count").help("The number of nodes to spin up").default_value("10"))
        .arg(Arg::with_name("node_path").long("node_path").takes_value(true).help( "The path to the nano_node to test"))
        .arg(Arg::with_name("rpc_path").long("rpc_path").takes_value(true).help("The path to the nano_rpc to test"))
        .arg(Arg::with_name("destination_count").long("destination_count").takes_value(true).default_value("2").help("How many destination accounts to choose between"))
        .arg(Arg::with_name("send_count").short("s").long("send_count").takes_value(true).default_value("2000").help("How many send blocks to generate"))
        .arg(Arg::with_name("simultaneous_process_calls").long("simultaneous_process_calls").takes_value(true).value_name("count").default_value("20").help("Number of simultaneous rpc sends to do"))
        .get_matches();

        let node_count = matches.value_of("node_count").unwrap().parse::<usize>()?;

        let destination_count = matches
            .value_of("destination_count")
            .unwrap()
            .parse::<usize>()?;

        let send_count = matches.value_of("send_count").unwrap().parse::<usize>()?;

        let simultaneous_process_calls = matches
            .value_of("simultaneous_process_calls")
            .unwrap()
            .parse::<usize>()?;

        let node_path = match matches.value_of("node_path") {
            Some(p) => p.into(),
            None => default_nano_executable("nano_node")?,
        };

        let rpc_path = match matches.value_of("rpc_path") {
            Some(p) => p.into(),
            None => default_nano_executable("nano_rpc")?,
        };

        let args = ProgramArgs {
            node_count,
            destination_count,
            send_count,
            simultaneous_process_calls,
            node_path,
            rpc_path,
        };

        Ok(args)
    }

    pub fn validate_paths(&self) -> Result<()> {
        if !self.node_path.exists() {
            bail!(
                "nano_node executable could not be found in {:?}",
                self.node_path
            );
        }

        if !self.rpc_path.exists() {
            bail!(
                "nano_rpc executable could not be found in {:?}",
                self.rpc_path
            );
        }

        Ok(())
    }
}

fn default_nano_executable(filename: impl AsRef<Path>) -> Result<PathBuf> {
    let running_executable_filepath = std::env::current_exe()?;
    let mut node_filepath = running_executable_filepath.clone();
    node_filepath.pop(); // debug
    node_filepath.pop(); // build
    node_filepath.pop(); // cargo
    node_filepath.pop(); // project root
    node_filepath.push(filename);
    if let Some(ext) = running_executable_filepath.extension() {
        node_filepath.set_extension(ext);
    }
    Ok(node_filepath)
}
