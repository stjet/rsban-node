use super::{
    ConfirmationJsonOptions, ConfirmationOptions, Message, MessageBuilder, Options, Topic,
    WebsocketSessionEntry,
};
use crate::{
    consensus::ElectionStatus, utils::AsyncRuntime, wallets::Wallets, websocket::WebsocketSession,
};
use futures_util::{SinkExt, StreamExt};
use rsnano_core::{Account, Amount, BlockEnum, VoteWithWeightInfo};
use std::{
    borrow::Cow,
    net::SocketAddr,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex, Weak,
    },
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{mpsc, oneshot},
};
use tokio_tungstenite::tungstenite::protocol::{frame::coding::CloseCode, CloseFrame};
use tracing::{info, warn};

pub struct WebsocketListener {
    endpoint: Mutex<SocketAddr>,
    tx_stop: Mutex<Option<oneshot::Sender<()>>>,
    wallets: Arc<Wallets>,
    topic_subscriber_count: Arc<[AtomicUsize; 11]>,
    sessions: Arc<Mutex<Vec<Weak<WebsocketSessionEntry>>>>,
    async_rt: Arc<AsyncRuntime>,
}

impl WebsocketListener {
    pub fn new(endpoint: SocketAddr, wallets: Arc<Wallets>, async_rt: Arc<AsyncRuntime>) -> Self {
        Self {
            endpoint: Mutex::new(endpoint),
            tx_stop: Mutex::new(None),
            wallets,
            topic_subscriber_count: Arc::new(std::array::from_fn(|_| AtomicUsize::new(0))),
            sessions: Arc::new(Mutex::new(Vec::new())),
            async_rt,
        }
    }

    pub fn any_subscriber(&self, topic: Topic) -> bool {
        self.subscriber_count(topic) > 0
    }

    pub fn subscriber_count(&self, topic: Topic) -> usize {
        self.topic_subscriber_count[topic as usize].load(Ordering::SeqCst)
    }

    async fn run(&self) {
        let endpoint = self.endpoint.lock().unwrap().clone();
        let listener = match TcpListener::bind(endpoint).await {
            Ok(s) => s,
            Err(e) => {
                warn!("Listen failed: {:?}", e);
                return;
            }
        };
        let ep = listener.local_addr().unwrap();
        *self.endpoint.lock().unwrap() = ep;
        info!("Websocket listener started on {}", ep);

        let (tx_stop, rx_stop) = oneshot::channel::<()>();
        *self.tx_stop.lock().unwrap() = Some(tx_stop);

        tokio::select! {
            _ = rx_stop =>{},
           _ = self.accept(listener) =>{}
        }
    }

    /// Close all websocket sessions and stop listening for new connections
    pub async fn stop_async(&self) {
        if let Some(tx) = self.tx_stop.lock().unwrap().take() {
            tx.send(()).unwrap()
        }

        let mut sessions = self.sessions.lock().unwrap();
        for session in sessions.drain(..) {
            if let Some(session) = session.upgrade() {
                session.close();
            }
        }
    }

    pub fn listening_port(&self) -> u16 {
        self.endpoint.lock().unwrap().port()
    }

    /// Broadcast \p message to all session subscribing to the message topic.
    pub fn broadcast(&self, message: &Message) {
        let sessions = self.sessions.lock().unwrap();
        for session in sessions.iter() {
            if let Some(session) = session.upgrade() {
                let _ = session.blocking_write(message.clone());
            }
        }
    }

    /// Broadcast block confirmation. The content of the message depends on subscription options (such as "include_block")
    pub fn broadcast_confirmation(
        &self,
        block_a: &Arc<BlockEnum>,
        account_a: &Account,
        amount_a: &Amount,
        subtype: &str,
        election_status_a: &ElectionStatus,
        election_votes_a: &Vec<VoteWithWeightInfo>,
    ) {
        let mut msg_with_block = None;
        let mut msg_without_block = None;
        let sessions = self.sessions.lock().unwrap();
        for session in sessions.iter() {
            if let Some(session) = session.upgrade() {
                let subs = session.subscriptions.lock().unwrap();
                if let Some(options) = subs.get(&Topic::Confirmation) {
                    let default_opts = ConfirmationOptions::new(
                        Arc::clone(&self.wallets),
                        ConfirmationJsonOptions::default(),
                    );
                    let conf_opts = if let Options::Confirmation(i) = options {
                        i
                    } else {
                        &default_opts
                    };

                    let include_block = conf_opts.include_block;

                    if include_block && msg_with_block.is_none() {
                        msg_with_block = Some(
                            MessageBuilder::block_confirmed(
                                block_a,
                                account_a,
                                amount_a,
                                subtype.to_string(),
                                include_block,
                                election_status_a,
                                election_votes_a,
                                conf_opts,
                            )
                            .unwrap(),
                        );
                    } else if !include_block && msg_without_block.is_none() {
                        msg_without_block = Some(
                            MessageBuilder::block_confirmed(
                                block_a,
                                account_a,
                                amount_a,
                                subtype.to_string(),
                                include_block,
                                election_status_a,
                                election_votes_a,
                                conf_opts,
                            )
                            .unwrap(),
                        );
                    }
                    drop(subs);
                    let _ = session.blocking_write(if include_block {
                        msg_with_block.as_ref().unwrap().clone()
                    } else {
                        msg_without_block.as_ref().unwrap().clone()
                    });
                }
            }
        }
    }

    async fn accept(&self, listener: TcpListener) {
        loop {
            match listener.accept().await {
                Ok((stream, remote_endpoint)) => {
                    let wallets = Arc::clone(&self.wallets);
                    let sub_count = Arc::clone(&self.topic_subscriber_count);
                    let (tx_send, rx_send) = mpsc::channel::<Message>(1024);
                    let sessions = Arc::clone(&self.sessions);
                    tokio::spawn(async move {
                        if let Err(e) = accept_connection(
                            stream,
                            wallets,
                            sub_count,
                            remote_endpoint,
                            tx_send,
                            rx_send,
                            sessions,
                        )
                        .await
                        {
                            warn!("listener failed: {:?}", e)
                        }
                    });
                }
                Err(e) => warn!("Accept failed: {:?}", e),
            }
        }
    }
}

pub trait WebsocketListenerExt {
    fn start(&self);
    fn stop(&self);
}

impl WebsocketListenerExt for Arc<WebsocketListener> {
    /// Start accepting connections
    fn start(&self) {
        let self_l = Arc::clone(self);
        self.async_rt.tokio.spawn(async move {
            self_l.run().await;
        });
    }

    fn stop(&self) {
        let self_l = Arc::clone(self);
        self.async_rt.tokio.spawn(async move {
            self_l.stop_async().await;
        });
    }
}

async fn accept_connection(
    stream: TcpStream,
    wallets: Arc<Wallets>,
    topic_subscriber_count: Arc<[AtomicUsize; 11]>,
    remote_endpoint: SocketAddr,
    tx_send: mpsc::Sender<Message>,
    mut rx_send: mpsc::Receiver<Message>,
    sessions: Arc<Mutex<Vec<Weak<WebsocketSessionEntry>>>>,
) -> anyhow::Result<()> {
    // Create the session and initiate websocket handshake
    let mut ws_stream = tokio_tungstenite::accept_async(stream).await?;

    let (tx_close, rx_close) = oneshot::channel::<()>();
    let entry = Arc::new(WebsocketSessionEntry::new(tx_send, tx_close));

    {
        let mut sessions = sessions.lock().unwrap();
        sessions.retain(|s| s.strong_count() > 0);
        sessions.push(Arc::downgrade(&entry));
    }

    let session = WebsocketSession::new(wallets, topic_subscriber_count, remote_endpoint, entry);

    tokio::select! {
        _ = rx_close =>{
            ws_stream
                .close(Some(CloseFrame {
                    code: CloseCode::Normal,
                    reason: Cow::Borrowed("Shutting down"),
                }))
                .await?;
        }
        res = session.run(&mut ws_stream, &mut rx_send) =>{
            res?;
        }
    };

    Ok(())
}
