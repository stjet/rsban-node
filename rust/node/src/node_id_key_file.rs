use crate::nullable_fs::NullableFilesystem;
use anyhow::Context;
use rsnano_core::{KeyPair, KeyPairFactory};
use std::path::{Path, PathBuf};
use tracing::info;

/// The node creates a file called 'node_id_private.key' on its first start.
/// In this file the private key of the node id is stored.
#[derive(Default)]
pub(crate) struct NodeIdKeyFile {
    key_factory: KeyPairFactory,
    fs: NullableFilesystem,
}

impl NodeIdKeyFile {
    fn new(fs: NullableFilesystem, key_factory: KeyPairFactory) -> Self {
        Self { fs, key_factory }
    }

    pub fn initialize(&mut self, app_path: impl AsRef<Path>) -> anyhow::Result<KeyPair> {
        let app_path = app_path.as_ref();
        let file_path = Self::key_file_path(app_path);
        if self.fs.exists(&file_path) {
            self.load_key(&file_path)
        } else {
            self.create_key(app_path, &file_path)
        }
    }

    fn key_file_path(app_path: &Path) -> PathBuf {
        let mut key_file = PathBuf::from(app_path);
        key_file.push("node_id_private.key");
        key_file
    }

    fn load_key(&mut self, file_path: &Path) -> anyhow::Result<KeyPair> {
        info!("Reading node id from: {:?}", file_path);
        let content = self
            .fs
            .read_to_string(&file_path)
            .context(format!("Could not read node id file {:?}", file_path))?;
        KeyPair::from_priv_key_hex(&content).context("Could not decode node id from file")
    }

    fn create_key(&mut self, app_path: &Path, file_path: &Path) -> anyhow::Result<KeyPair> {
        info!("Generating a new node id, saving to: {:?}", file_path);
        self.fs
            .create_dir_all(app_path)
            .expect("Could not create app dir");
        let keypair = self.key_factory.create_key_pair();
        self.fs
            .write(file_path, keypair.private_key().encode_hex().as_bytes())
            .expect("Could not write node id file");
        Ok(keypair)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nullable_fs::FsEvent;
    use rsnano_core::RawKey;
    use std::io::ErrorKind;
    use tracing_test::traced_test;

    #[test]
    fn node_id_key_file_path() {
        assert_eq!(
            NodeIdKeyFile::key_file_path(&PathBuf::from("/path/to/node")),
            PathBuf::from("/path/to/node/node_id_private.key")
        );
    }

    #[test]
    #[traced_test]
    fn read_existing_file() {
        let expected_key = RawKey::from(42);
        let fs = fs_with_key_file(expected_key.encode_hex());

        let (key_pair, fs_events) = initialize_node_id(fs, KeyPairFactory::new_null());

        assert_eq!(key_pair.unwrap().private_key(), expected_key);
        assert!(fs_events.is_empty());
        assert!(logs_contain(
            "Reading node id from: \"/path/to/node/node_id_private.key\""
        ));
    }

    #[test]
    #[traced_test]
    fn create_node_id_file_on_first_start() {
        let fs = NullableFilesystem::new_null();
        let expected_key = RawKey::from(42);
        let key_pair_factory = KeyPairFactory::new_null_with(expected_key);

        let (key_pair, fs_events) = initialize_node_id(fs, key_pair_factory);

        assert_eq!(key_pair.unwrap().private_key(), expected_key);
        assert_eq!(fs_events.len(), 2);
        assert_eq!(fs_events[0], FsEvent::create_dir_all(&test_app_path()));
        assert_eq!(
            fs_events[1],
            FsEvent::write(&test_key_file_path(), expected_key.encode_hex())
        );
        assert!(logs_contain(
            "Generating a new node id, saving to: \"/path/to/node/node_id_private.key\""
        ));
    }

    #[test]
    fn no_access_to_existing_file() {
        let fs = fs_with_inaccessable_key_file();
        let (Err(err), _) = initialize_node_id(fs, KeyPairFactory::new_null()) else {
            panic!("initialization should fail")
        };
        assert_eq!(
            err.to_string(),
            "Could not read node id file \"/path/to/node/node_id_private.key\""
        )
    }

    fn initialize_node_id(
        fs: NullableFilesystem,
        key_pair_factory: KeyPairFactory,
    ) -> (anyhow::Result<KeyPair>, Vec<FsEvent>) {
        let fs_tracker = fs.track();
        let mut id_file = NodeIdKeyFile::new(fs, key_pair_factory);
        let key_pair = id_file.initialize(&test_app_path());
        let fs_events = fs_tracker.output();
        (key_pair, fs_events)
    }

    fn test_app_path() -> PathBuf {
        PathBuf::from("/path/to/node")
    }

    fn test_key_file_path() -> PathBuf {
        NodeIdKeyFile::key_file_path(&test_app_path())
    }

    fn fs_with_key_file(contents: impl Into<String>) -> NullableFilesystem {
        let file_path = test_key_file_path();
        NullableFilesystem::null_builder()
            .path_exists(&file_path)
            .read_to_string(&file_path, contents.into())
            .finish()
    }

    fn fs_with_inaccessable_key_file() -> NullableFilesystem {
        let file_path = test_key_file_path();
        NullableFilesystem::null_builder()
            .path_exists(&file_path)
            .read_to_string_fails(
                &file_path,
                std::io::Error::new(ErrorKind::PermissionDenied, ""),
            )
            .finish()
    }
}
