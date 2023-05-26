use std::sync::Arc;

use crate::{as_write_txn, count, get, parallel_traversal_u512, LmdbEnv, LmdbIteratorImpl, EnvironmentStrategy, EnvironmentWrapper, iterator::DbIterator, WriteTransaction, Transaction, ReadTransaction};
use lmdb::{Database, DatabaseFlags, WriteFlags};
use rsnano_core::{BlockHash, QualifiedRoot, Root};

pub type FinalVoteIterator = Box<dyn DbIterator<QualifiedRoot, BlockHash>>;

/// Maps root to block hash for generated final votes.
/// nano::qualified_root -> nano::block_hash
pub struct LmdbFinalVoteStore<T: EnvironmentStrategy = EnvironmentWrapper> {
    env: Arc<LmdbEnv<T>>,
    database: Database,
}

impl<T:EnvironmentStrategy + 'static> LmdbFinalVoteStore<T> {
    pub fn new(env: Arc<LmdbEnv<T>>) -> anyhow::Result<Self> {
        let database = env
            .environment
            .create_db(Some("final_votes"), DatabaseFlags::empty())?;
        Ok(Self { env, database })
    }

    pub fn database(&self) -> Database {
        self.database
    }

    pub fn put(&self, txn: &mut dyn WriteTransaction, root: &QualifiedRoot, hash: &BlockHash) -> bool {
        let root_bytes = root.to_bytes();
        match get::<T, _>(txn.txn(), self.database, &root_bytes) {
            Err(lmdb::Error::NotFound) => {
                as_write_txn::<T>(txn)
                    .put(
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
        LmdbIteratorImpl::new_iterator::<T, _, _>(txn, self.database, None, true)
    }

    pub fn begin_at_root(&self, txn: &dyn Transaction, root: &QualifiedRoot) -> FinalVoteIterator {
        let key_bytes = root.to_bytes();
        LmdbIteratorImpl::new_iterator::<T, _, _>(txn, self.database, Some(&key_bytes), true)
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

    pub fn del(&self, txn: &mut dyn WriteTransaction, root: &Root) {
        let mut final_vote_qualified_roots = Vec::new();

        let mut it = self.begin_at_root(
            txn.txn(),
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
            as_write_txn::<T>(txn)
                .del(self.database, &root_bytes, None)
                .unwrap();
        }
    }

    pub fn count(&self, txn: &dyn Transaction) -> u64 {
        count::<T>(txn, self.database)
    }

    pub fn clear(&self, txn: &mut dyn WriteTransaction) {
        as_write_txn::<T>(txn).clear_db(self.database).unwrap();
    }

    pub fn for_each_par(
        &self,
        action: &(dyn Fn(&dyn ReadTransaction, FinalVoteIterator, FinalVoteIterator) + Send + Sync),
    ) {
        parallel_traversal_u512(&|start, end, is_last| {
            let transaction = self.env.tx_begin_read().unwrap();
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
    use crate::TestLmdbEnv;
    use primitive_types::U512;

    use super::*;

    #[test]
    fn del() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbFinalVoteStore::new(env.env())?;
        let mut txn = env.tx_begin_write()?;
        let root1 = QualifiedRoot::from(U512::from(1));
        let root2 = QualifiedRoot::from(U512::MAX);
        store.put(&mut txn, &root1, &BlockHash::from(3));
        store.put(&mut txn, &root2, &BlockHash::from(4));

        store.del(&mut txn, &root1.root);

        assert_eq!(store.count(&txn), 1);
        assert_eq!(store.get(&txn, root1.root).len(), 0);
        Ok(())
    }

    #[test]
    fn del_unknown_root_should_not_remove() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbFinalVoteStore::new(env.env())?;
        let mut txn = env.tx_begin_write()?;
        let root1 = QualifiedRoot::from(U512::from(1));
        let root2 = QualifiedRoot::from(U512::MAX);
        store.put(&mut txn, &root1, &BlockHash::from(3));

        store.del(&mut txn, &root2.root);

        assert_eq!(store.count(&txn), 1);
        Ok(())
    }

    #[test]
    fn clear() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbFinalVoteStore::new(env.env())?;
        let mut txn = env.tx_begin_write()?;
        let root1 = QualifiedRoot::from(U512::from(1));
        let root2 = QualifiedRoot::from(U512::MAX);
        store.put(&mut txn, &root1, &BlockHash::from(3));
        store.put(&mut txn, &root2, &BlockHash::from(4));

        store.clear(&mut txn);

        assert_eq!(store.count(&txn), 0);
        Ok(())
    }
}
