mod change;
mod open;
mod receive;
mod saved_account_chain;
mod send;
mod state;

pub use change::TestLegacyChangeBlockBuilder;
pub use open::TestLegacyOpenBlockBuilder;
pub use receive::TestLegacyReceiveBlockBuilder;
pub use saved_account_chain::SavedAccountChain;
pub use send::TestLegacySendBlockBuilder;
pub use state::TestStateBlockBuilder;

pub struct TestBlockBuilder {}

impl TestBlockBuilder {
    pub fn state() -> TestStateBlockBuilder {
        TestStateBlockBuilder::new()
    }

    pub fn legacy_open() -> TestLegacyOpenBlockBuilder {
        TestLegacyOpenBlockBuilder::new()
    }

    pub fn legacy_receive() -> TestLegacyReceiveBlockBuilder {
        TestLegacyReceiveBlockBuilder::new()
    }

    pub fn legacy_send() -> TestLegacySendBlockBuilder {
        TestLegacySendBlockBuilder::new()
    }

    pub fn legacy_change() -> TestLegacyChangeBlockBuilder {
        TestLegacyChangeBlockBuilder::new()
    }
}
