use crate::{
    datastore::{
        lmdb::{MDB_NOTFOUND, MDB_SUCCESS},
        BlockStore, Transaction, WriteTransaction,
    },
    utils::{MemoryStream, Stream},
    Block, BlockHash, BlockSideband, BlockType, BlockVisitor,
};
use num_traits::FromPrimitive;
use std::{ffi::c_void, sync::Arc};

use super::{
    assert_success, get_raw_lmdb_txn, mdb_get, mdb_put, LmdbEnv, LmdbWriteTransaction, MdbVal,
};

pub struct LmdbBlockStore {
    env: Arc<LmdbEnv>,
    pub blocks_handle: u32,
}

impl LmdbBlockStore {
    pub fn new(env: Arc<LmdbEnv>) -> Self {
        Self {
            env,
            blocks_handle: 0,
        }
    }

    pub fn raw_put(&self, txn: &LmdbWriteTransaction, data: &[u8], hash: &BlockHash) {
        let mut key = MdbVal::from_slice(hash.as_bytes());
        let mut data = MdbVal::from_slice(data);
        let status = unsafe { mdb_put(txn.handle, self.blocks_handle, &mut key, &mut data, 0) };
        assert_success(status);
    }

    pub fn block_raw_get(&self, txn: &dyn Transaction, hash: &BlockHash, value: &mut MdbVal) {
        let mut key = MdbVal::from_slice(hash.as_bytes());
        let status = unsafe { mdb_get(get_raw_lmdb_txn(txn), self.blocks_handle, &mut key, value) };
        assert!(status == MDB_SUCCESS || status == MDB_NOTFOUND);
    }
}

unsafe fn block_type_from_raw(data: *const c_void) -> Option<BlockType> {
    // The block type is the first byte
    let first_byte = *(data as *const u8);
    BlockType::from_u8(first_byte)
}

impl BlockStore for LmdbBlockStore {
    fn put(&self, txn: &dyn WriteTransaction, hash: &BlockHash, block: &dyn Block) {
        debug_assert!(
            block.sideband().unwrap().successor.is_zero()
                || self.exists(txn.as_transaction(), &block.sideband().unwrap().successor)
        );

        let lmdb_txn = txn.as_any().downcast_ref::<LmdbWriteTransaction>().unwrap();
        let mut stream = MemoryStream::new();
        stream.write_u8(block.block_type() as u8).unwrap();
        block.serialize(&mut stream).unwrap();
        block
            .sideband()
            .unwrap()
            .serialize(&mut stream, block.block_type())
            .unwrap();
        self.raw_put(lmdb_txn, stream.as_bytes(), hash);
        let mut predecessor = BlockPredecessorMdbSet::new(lmdb_txn, self);
        block.visit(&mut predecessor);

        debug_assert!(
            block.previous().is_zero()
                || self.successor(txn.as_transaction(), block.previous()) == *hash
        );
    }

    fn exists(&self, transaction: &dyn Transaction, hash: &BlockHash) -> bool {
        let mut junk = MdbVal::new();
        self.block_raw_get(transaction, hash, &mut junk);
        junk.mv_size != 0
    }

    fn successor(&self, txn: &dyn Transaction, hash: &BlockHash) -> BlockHash {
        let mut value = MdbVal::new();
        self.block_raw_get(txn, hash, &mut value);
        let data = value.as_slice();
        if data.len() != 0 {
            debug_assert!(data.len() >= 32);
            let block_type = BlockType::from_u8(data[0]).unwrap();
            let offset = block_successor_offset(data.len(), block_type);
            BlockHash::from_bytes(data[offset..offset + 32].try_into().unwrap())
        } else {
            BlockHash::new()
        }
    }

    fn successor_clear(&self, txn: &dyn WriteTransaction, hash: &BlockHash) {
        let mut value = MdbVal::new();
        self.block_raw_get(txn.as_transaction(), hash, &mut value);
        debug_assert!(value.mv_size != 0);
        let block_type = unsafe { block_type_from_raw(value.mv_data) }.unwrap();

        let mut data =
            unsafe { std::slice::from_raw_parts(value.mv_data as *const u8, value.mv_size) }
                .to_vec();
        let offset = block_successor_offset(value.mv_size, block_type);
        data[offset..offset + BlockHash::serialized_size()].fill(0);
        let lmdb_txn = txn.as_any().downcast_ref::<LmdbWriteTransaction>().unwrap();
        self.raw_put(lmdb_txn, &data, hash)
    }
}

/// Fill in our predecessors
struct BlockPredecessorMdbSet<'a> {
    transaction: &'a LmdbWriteTransaction,
    block_store: &'a LmdbBlockStore,
}

impl<'a> BlockPredecessorMdbSet<'a> {
    fn new(transaction: &'a LmdbWriteTransaction, block_store: &'a LmdbBlockStore) -> Self {
        Self {
            transaction,
            block_store,
        }
    }

    fn fill_value(&mut self, block: &dyn Block) {
        let hash = block.hash();
        let mut value = MdbVal::new();
        self.block_store
            .block_raw_get(self.transaction, block.previous(), &mut value);
        debug_assert!(value.mv_size != 0);
        let mut data = value.as_slice().to_vec();
        let block_type = BlockType::from_u8(data[0]).unwrap();

        let offset = block_successor_offset(data.len(), block_type);
        data[offset..offset + hash.as_bytes().len()].copy_from_slice(hash.as_bytes());

        self.block_store
            .raw_put(self.transaction, &data, block.previous());
    }
}

impl<'a> BlockVisitor for BlockPredecessorMdbSet<'a> {
    fn send_block(&mut self, block: &crate::SendBlock) {
        self.fill_value(block);
    }

    fn receive_block(&mut self, block: &crate::ReceiveBlock) {
        self.fill_value(block);
    }

    fn open_block(&mut self, _block: &crate::OpenBlock) {
        // Open blocks don't have a predecessor
    }

    fn change_block(&mut self, block: &crate::ChangeBlock) {
        self.fill_value(block);
    }

    fn state_block(&mut self, block: &crate::StateBlock) {
        if !block.previous().is_zero() {
            self.fill_value(block);
        }
    }
}

fn block_successor_offset(entry_size: usize, block_type: BlockType) -> usize {
    entry_size - BlockSideband::serialized_size(block_type)
}
