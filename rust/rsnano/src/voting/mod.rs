mod vote;
use std::sync::RwLock;

pub(crate) use vote::*;

use crate::Uniquer;

pub(crate) type VoteUniquer = Uniquer<RwLock<Vote>>;