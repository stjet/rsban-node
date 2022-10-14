mod open_block_builder;
mod receive_block_builder;
mod state_block_builder;

pub use open_block_builder::OpenBlockBuilder;
pub use receive_block_builder::ReceiveBlockBuilder;
pub use state_block_builder::StateBlockBuilder;

pub struct BlockBuilder {}

impl BlockBuilder {
    pub fn state() -> StateBlockBuilder {
        StateBlockBuilder::new()
    }

    pub fn open() -> OpenBlockBuilder {
        OpenBlockBuilder::new()
    }

    pub fn receive() -> ReceiveBlockBuilder {
        ReceiveBlockBuilder::new()
    }
}
