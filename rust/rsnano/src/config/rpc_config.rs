use anyhow::Result;
use std::path::{Path, PathBuf};

pub fn get_default_rpc_filepath() -> Result<PathBuf> {
    Ok(get_default_rpc_filepath_from(
        std::env::current_exe()?.as_path(),
    ))
}

fn get_default_rpc_filepath_from(node_exe_path: &Path) -> PathBuf {
    let mut result = node_exe_path.to_path_buf();
    result.pop();
    result.push("nano_rpc");
    if let Some(ext) = node_exe_path.extension(){
        result.set_extension(ext);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_rpc_filepath() -> Result<()> {
        assert_eq!(
            get_default_rpc_filepath_from(Path::new("/path/to/nano_node")),
            Path::new("/path/to/nano_rpc")
        );

        assert_eq!(
            get_default_rpc_filepath_from(Path::new("/nano_node")),
            Path::new("/nano_rpc")
        );

        assert_eq!(
            get_default_rpc_filepath_from(Path::new("/bin/nano_node.exe")),
            Path::new("/bin/nano_rpc.exe")
        );

        Ok(())
    }
}
