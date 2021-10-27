use crate::numbers::Signature;

#[derive(Clone)]
pub struct ReceiveBlock {
    pub work: u64,
    pub signature: Signature,
}
