use std::sync::Arc;

use crate::network::TcpMessageItem;

pub struct TcpMessageItemHandle(Arc<TcpMessageItem>);
