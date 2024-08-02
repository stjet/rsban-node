use super::{NodeConfig, NodeRpcConfig, OpenclConfig};
use crate::NetworkParams;
use anyhow::Result;
use rsnano_core::utils::{get_cpu_count, TomlWriter};

pub struct DaemonConfig {
    pub rpc_enable: bool,
    pub rpc: NodeRpcConfig,
    pub node: NodeConfig,
    pub opencl: OpenclConfig,
    pub opencl_enable: bool,
}

impl DaemonConfig {
    pub fn new(network_params: &NetworkParams, parallelism: usize) -> Self {
        Self {
            rpc_enable: false,
            node: NodeConfig::new(
                Some(network_params.network.default_node_port),
                network_params,
                parallelism,
            ),
            opencl: OpenclConfig::new(),
            opencl_enable: false,
            rpc: NodeRpcConfig::new(),
        }
    }

    pub fn serialize_toml(&self, toml: &mut dyn TomlWriter) -> Result<()> {
        toml.put_child("rpc", &mut |rpc| {
            self.rpc.serialize_toml(rpc)?;
            rpc.put_bool(
                "enable",
                self.rpc_enable,
                "Enable or disable RPC\ntype:bool",
            )?;
            Ok(())
        })?;

        toml.put_child("node", &mut |node| self.node.serialize_toml(node))?;

        toml.put_child("opencl", &mut |opencl| {
            self.opencl.serialize_toml(opencl)?;
            opencl.put_bool(
                "enable",
                self.opencl_enable,
                "Enable or disable OpenCL work generation\nIf enabled, consider freeing up CPU resources by setting [work_threads] to zero\ntype:bool",
            )?;
            Ok(())
        })?;

        Ok(())
    }
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self::new(&NetworkParams::default(), get_cpu_count())
    }
}
