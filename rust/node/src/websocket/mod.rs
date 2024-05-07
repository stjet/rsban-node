mod confirmation_options;
mod listener;
mod message;
mod options;
mod vote_options;
mod websocket_server;
mod websocket_session;

pub use confirmation_options::*;
pub use listener::*;
pub use message::*;
pub use options::*;
use serde::Deserialize;
use serde_json::Value;
pub use vote_options::*;
pub use websocket_server::*;
pub use websocket_session::*;

#[derive(Deserialize)]
pub struct IncomingMessage<'a> {
    pub action: Option<&'a str>,
    pub topic: Option<&'a str>,
    #[serde(default)]
    pub ack: bool,
    pub id: Option<&'a str>,
    pub options: Option<Value>,
    #[serde(default)]
    pub accounts_add: Vec<&'a str>,
    #[serde(default)]
    pub accounts_del: Vec<&'a str>,
}
