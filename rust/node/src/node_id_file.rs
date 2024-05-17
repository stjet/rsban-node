use std::path::{Path, PathBuf};

use rsnano_core::{KeyPair, KeyPairFactory};
use tracing::info;

use crate::nullable_fs::NullableFilesystem;

/// The node creates a file called 'node_id_private.key' on its first start.
/// In this file the private key of the node id is stored.
#[derive(Default)]
pub(crate) struct NodeIdFile {
    key_factory: KeyPairFactory,
    fs: NullableFilesystem,
}

impl NodeIdFile {
    fn new(fs: NullableFilesystem, key_factory: KeyPairFactory) -> Self {
        Self { fs, key_factory }
    }

    pub fn initialize(&mut self, path: impl AsRef<Path>) -> KeyPair {
        let path = path.as_ref();
        let mut private_key_path = PathBuf::from(path);
        private_key_path.push("node_id_private.key");
        if self.fs.exists(&private_key_path) {
            info!("Reading node id from: {:?}", private_key_path);
            let content = self
                .fs
                .read_to_string(&private_key_path)
                .expect("Could not read node id file");
            KeyPair::from_priv_key_hex(&content).expect("Could not read node id")
        } else {
            self.fs
                .create_dir_all(path)
                .expect("Could not create app dir");
            info!(
                "Generating a new node id, saving to: {:?}",
                private_key_path
            );
            let keypair = self.key_factory.create_key_pair();
            self.fs
                .write(
                    private_key_path,
                    keypair.private_key().encode_hex().as_bytes(),
                )
                .expect("Could not write node id file");
            keypair
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::nullable_fs::FsEvent;

    use super::*;
    use rsnano_core::RawKey;
    use tracing_test::traced_test;

    #[test]
    #[traced_test]
    fn read_existing_file() {
        let app_path = PathBuf::from("/path/to/node");
        let file_path = PathBuf::from("/path/to/node/node_id_private.key");
        let fs = NullableFilesystem::null_builder()
            .path_exists(&file_path)
            .read_to_string(
                &file_path,
                "34F9A0542BCFC0139C7CF57524F5D52BD8141F69E83CF05578C8E27EE4942892".to_string(),
            )
            .finish();
        let fs_tracker = fs.track();
        let key_pair_factory = KeyPairFactory::new_null();
        let mut id_file = NodeIdFile::new(fs, key_pair_factory);

        let key_pair = id_file.initialize(&app_path);

        assert_eq!(
            key_pair.private_key(),
            RawKey::from_bytes([
                0x34, 0xF9, 0xA0, 0x54, 0x2B, 0xCF, 0xC0, 0x13, 0x9C, 0x7C, 0xF5, 0x75, 0x24, 0xF5,
                0xD5, 0x2B, 0xD8, 0x14, 0x1F, 0x69, 0xE8, 0x3C, 0xF0, 0x55, 0x78, 0xC8, 0xE2, 0x7E,
                0xE4, 0x94, 0x28, 0x92
            ])
        );

        assert!(fs_tracker.output().is_empty());
        assert!(logs_contain(
            "Reading node id from: \"/path/to/node/node_id_private.key\""
        ));
    }

    #[test]
    #[traced_test]
    fn create_node_id_file_on_first_start() {
        let app_path = PathBuf::from("/path/to/node");
        let file_path = PathBuf::from("/path/to/node/node_id_private.key");
        let fs = NullableFilesystem::new_null();
        let fs_tracker = fs.track();
        let expected_key = RawKey::from_bytes([0x2A; 32]);
        let key_pair_factory = KeyPairFactory::new_null_with(expected_key);
        let mut id_file = NodeIdFile::new(fs, key_pair_factory);

        let key_pair = id_file.initialize(&app_path);

        assert_eq!(key_pair.private_key(), expected_key);

        let output = fs_tracker.output();
        assert_eq!(output.len(), 2);
        assert_eq!(output[0], FsEvent::create_dir_all(&app_path));
        assert_eq!(
            output[1],
            FsEvent::write(
                &file_path,
                b"2A2A2A2A2A2A2A2A2A2A2A2A2A2A2A2A2A2A2A2A2A2A2A2A2A2A2A2A2A2A2A2A"
            )
        );
        assert!(logs_contain(
            "Generating a new node id, saving to: \"/path/to/node/node_id_private.key\""
        ));
    }
}
