use std::sync::Arc;

use lmdb::{Database, DatabaseFlags, Transaction, WriteFlags};

use crate::{
    datastore::{final_vote_store::FinalVoteIterator, parallel_traversal_u512, FinalVoteStore},
    BlockHash, QualifiedRoot, Root,
};

use super::{
    LmdbEnv, LmdbIteratorImpl, LmdbReadTransaction, LmdbTransaction, LmdbWriteTransaction,
};

/// Maps root to block hash for generated final votes.
/// nano::qualified_root -> nano::block_hash
pub struct LmdbFinalVoteStore {
    env: Arc<LmdbEnv>,
    database: Database,
}

impl LmdbFinalVoteStore {
    pub fn new(env: Arc<LmdbEnv>) -> anyhow::Result<Self> {
        let database = env
            .environment
            .create_db(Some("final_votes"), DatabaseFlags::empty())?;
        Ok(Self { env, database })
    }

    pub fn database(&self) -> Database {
        self.database
    }
}

impl<'a> FinalVoteStore<'a, LmdbReadTransaction<'a>, LmdbWriteTransaction<'a>, LmdbIteratorImpl>
    for LmdbFinalVoteStore
{
    fn put(&self, txn: &mut LmdbWriteTransaction, root: &QualifiedRoot, hash: &BlockHash) -> bool {
        let root_bytes = root.to_bytes();
        match txn.rw_txn().get(self.database, &root_bytes) {
            Err(lmdb::Error::NotFound) => {
                txn.rw_txn_mut()
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

    fn begin(&self, txn: &LmdbTransaction) -> FinalVoteIterator<LmdbIteratorImpl> {
        FinalVoteIterator::new(LmdbIteratorImpl::new(txn, self.database, None, true))
    }

    fn begin_at_root(
        &self,
        txn: &LmdbTransaction,
        root: &QualifiedRoot,
    ) -> FinalVoteIterator<LmdbIteratorImpl> {
        let key_bytes = root.to_bytes();
        FinalVoteIterator::new(LmdbIteratorImpl::new(
            txn,
            self.database,
            Some(&key_bytes),
            true,
        ))
    }

    fn get(&self, txn: &LmdbTransaction, root: Root) -> Vec<BlockHash> {
        let mut result = Vec::new();
        let key_start = QualifiedRoot {
            root,
            previous: BlockHash::new(),
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

    fn del(&self, txn: &mut LmdbWriteTransaction, root: Root) {
        let mut final_vote_qualified_roots = Vec::new();

        let mut it = self.begin_at_root(
            &txn.as_txn(),
            &QualifiedRoot {
                root,
                previous: BlockHash::new(),
            },
        );
        while let Some((k, _)) = it.current() {
            if k.root != root {
                break;
            }
            final_vote_qualified_roots.push(k.clone());
            it.next();
        }

        for qualified_root in final_vote_qualified_roots {
            let root_bytes = qualified_root.to_bytes();
            txn.rw_txn_mut()
                .del(self.database, &root_bytes, None)
                .unwrap();
        }
    }

    fn count(&self, txn: &LmdbTransaction) -> usize {
        txn.count(self.database)
    }

    fn clear(&self, txn: &mut LmdbWriteTransaction) {
        txn.rw_txn_mut().clear_db(self.database).unwrap();
    }

    fn for_each_par(
        &'a self,
        action: &(dyn Fn(
            LmdbReadTransaction<'a>,
            FinalVoteIterator<LmdbIteratorImpl>,
            FinalVoteIterator<LmdbIteratorImpl>,
        ) + Send
              + Sync),
    ) {
        parallel_traversal_u512(&|start, end, is_last| {
            let transaction = self.env.tx_begin_read().unwrap();
            let begin_it = self.begin_at_root(&transaction.as_txn(), &start.into());
            let end_it = if !is_last {
                self.begin_at_root(&transaction.as_txn(), &end.into())
            } else {
                self.end()
            };
            action(transaction, begin_it, end_it);
        });
    }

    fn end(&self) -> FinalVoteIterator<LmdbIteratorImpl> {
        FinalVoteIterator::new(LmdbIteratorImpl::null())
    }
}
