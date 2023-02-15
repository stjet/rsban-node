use std::path::Path;

use crate::{
    AccountStore, BlockStore, ConfirmationHeightStore, FinalVoteStore, FrontierStore,
    OnlineWeightStore, PeerStore, PendingStore, PrunedStore, ReadTransaction, UncheckedStore,
    WriteTransaction,
};

pub trait Store: Send + Sync {
    fn tx_begin_read(&self) -> Box<dyn ReadTransaction>;
    fn tx_begin_write(&self) -> Box<dyn WriteTransaction>;
    fn tx_begin_write_for(&self, to_lock: &[Table]) -> Box<dyn WriteTransaction>;
    fn copy_db(&self, destination: &Path) -> anyhow::Result<()>;
    fn account(&self) -> &dyn AccountStore;
    fn confirmation_height(&self) -> &dyn ConfirmationHeightStore;
    fn pruned(&self) -> &dyn PrunedStore;
    fn block(&self) -> &dyn BlockStore;
    fn pending(&self) -> &dyn PendingStore;
    fn frontier(&self) -> &dyn FrontierStore;
    fn online_weight(&self) -> &dyn OnlineWeightStore;
    fn peers(&self) -> &dyn PeerStore;
    fn final_votes(&self) -> &dyn FinalVoteStore;
    fn unchecked(&self) -> &dyn UncheckedStore;
}

pub enum Table {
    ConfirmationHeight,
}
