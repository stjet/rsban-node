use crate::{
    iterator::DbIterator, parallel_traversal_u512, LmdbDatabase, LmdbEnv, LmdbIteratorImpl,
    LmdbReadTransaction, LmdbWriteTransaction, Transaction,
};
use lmdb::{DatabaseFlags, WriteFlags};
use rsnano_core::{BlockHash, QualifiedRoot, Root};
use std::sync::Arc;

pub type FinalVoteIterator = Box<dyn DbIterator<QualifiedRoot, BlockHash>>;

/// Maps root to block hash for generated final votes.
/// nano::qualified_root -> nano::block_hash
pub struct LmdbFinalVoteStore {
    env: Arc<LmdbEnv>,
    database: LmdbDatabase,
}

impl LmdbFinalVoteStore {
    pub fn new(env: Arc<LmdbEnv>) -> anyhow::Result<Self> {
        let database = env
            .environment
            .create_db(Some("final_votes"), DatabaseFlags::empty())?;
        Ok(Self { env, database })
    }

    pub fn database(&self) -> LmdbDatabase {
        self.database
    }

    pub fn put(
        &self,
        txn: &mut LmdbWriteTransaction,
        root: &QualifiedRoot,
        hash: &BlockHash,
    ) -> bool {
        let root_bytes = root.to_bytes();
        match txn.get(self.database, &root_bytes) {
            Err(lmdb::Error::NotFound) => {
                txn.put(
                    self.database,
                    &root_bytes,
                    hash.as_bytes(),
                    WriteFlags::empty(),
                )
                .unwrap();
                true
            }
            Ok(bytes) => BlockHash::from_slice(bytes).unwrap() == *hash,
            Err(e) => {
                panic!("Could not get final vote: {:?}", e);
            }
        }
    }

    pub fn begin(&self, txn: &dyn Transaction) -> FinalVoteIterator {
        LmdbIteratorImpl::new_iterator(txn, self.database, None, true)
    }

    pub fn begin_at_root(&self, txn: &dyn Transaction, root: &QualifiedRoot) -> FinalVoteIterator {
        let key_bytes = root.to_bytes();
        LmdbIteratorImpl::new_iterator(txn, self.database, Some(&key_bytes), true)
    }

    pub fn get(&self, txn: &dyn Transaction, root: Root) -> Vec<BlockHash> {
        let mut result = Vec::new();
        let key_start = QualifiedRoot {
            root,
            previous: BlockHash::zero(),
        };

        let mut i = self.begin_at_root(txn, &key_start);
        while let Some((k, v)) = i.current() {
            if k.root != root {
                break;
            }

            result.push(*v);
            i.next();
        }

        result
    }

    pub fn del(&self, txn: &mut LmdbWriteTransaction, root: &Root) {
        let mut final_vote_qualified_roots = Vec::new();

        let mut it = self.begin_at_root(
            txn,
            &QualifiedRoot {
                root: *root,
                previous: BlockHash::zero(),
            },
        );
        while let Some((k, _)) = it.current() {
            if k.root != *root {
                break;
            }
            final_vote_qualified_roots.push(k.clone());
            it.next();
        }

        for qualified_root in final_vote_qualified_roots {
            let root_bytes = qualified_root.to_bytes();
            txn.delete(self.database, &root_bytes, None).unwrap();
        }
    }

    pub fn count(&self, txn: &dyn Transaction) -> u64 {
        txn.count(self.database)
    }

    pub fn clear(&self, txn: &mut LmdbWriteTransaction) {
        txn.clear_db(self.database).unwrap();
    }

    pub fn for_each_par(
        &self,
        action: &(dyn Fn(&LmdbReadTransaction, FinalVoteIterator, FinalVoteIterator) + Send + Sync),
    ) {
        parallel_traversal_u512(&|start, end, is_last| {
            let transaction = self.env.tx_begin_read();
            let begin_it = self.begin_at_root(&transaction, &start.into());
            let end_it = if !is_last {
                self.begin_at_root(&transaction, &end.into())
            } else {
                self.end()
            };
            action(&transaction, begin_it, end_it);
        });
    }

    pub fn end(&self) -> FinalVoteIterator {
        LmdbIteratorImpl::null_iterator()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{lmdb_env::DatabaseStub, DeleteEvent};

    const TEST_DATABASE: DatabaseStub = DatabaseStub(100);

    struct Fixture {
        env: Arc<LmdbEnv>,
        store: LmdbFinalVoteStore,
    }

    impl Fixture {
        fn new() -> Self {
            Self::with_stored_entries(Vec::new())
        }

        fn with_stored_entries(entries: Vec<(QualifiedRoot, BlockHash)>) -> Self {
            let mut env = LmdbEnv::new_null_with().database("final_votes", TEST_DATABASE);
            for (key, value) in entries {
                env = env.entry(&key.to_bytes(), value.as_bytes());
            }
            Self::with_env(env.build().build())
        }

        fn with_env(env: LmdbEnv) -> Self {
            let env = Arc::new(env);
            Self {
                env: env.clone(),
                store: LmdbFinalVoteStore::new(env).unwrap(),
            }
        }
    }

    #[test]
    fn load() {
        let root = QualifiedRoot::create_test_instance();
        let hash = BlockHash::from(333);
        let fixture = Fixture::with_stored_entries(vec![(root.clone(), hash)]);
        let txn = fixture.env.tx_begin_read();

        let result = fixture.store.get(&txn, root.root);

        assert_eq!(result, vec![hash])
    }

    #[test]
    fn delete() {
        let root = QualifiedRoot::create_test_instance();
        let fixture = Fixture::with_stored_entries(vec![(root.clone(), BlockHash::from(333))]);
        let mut txn = fixture.env.tx_begin_write();
        let delete_tracker = txn.track_deletions();

        fixture.store.del(&mut txn, &root.root);

        assert_eq!(
            delete_tracker.output(),
            vec![DeleteEvent {
                key: root.to_bytes().to_vec(),
                database: TEST_DATABASE.into(),
            }]
        )
    }

    #[test]
    fn del_unknown_root_should_not_remove() {
        let fixture = Fixture::with_stored_entries(vec![(
            QualifiedRoot::create_test_instance(),
            BlockHash::from(333),
        )]);
        let mut txn = fixture.env.tx_begin_write();
        let delete_tracker = txn.track_deletions();

        fixture.store.del(&mut txn, &Root::from(98765));

        assert_eq!(delete_tracker.output(), vec![]);
    }

    #[test]
    fn clear() {
        let fixture = Fixture::new();
        let mut txn = fixture.env.tx_begin_write();
        let clear_tracker = txn.track_clears();

        fixture.store.clear(&mut txn);

        assert_eq!(clear_tracker.output(), vec![TEST_DATABASE.into()]);
    }
}
