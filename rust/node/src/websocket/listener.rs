use super::{to_topic, Message, MessageBuilder, Topic};
use crate::{consensus::ElectionStatus, utils::AsyncRuntime, wallets::Wallets};
use futures_util::{SinkExt, StreamExt};
use rsnano_core::{
    utils::{milliseconds_since_epoch, PropertyTree, SerdePropertyTree},
    Account, Amount, BlockEnum, VoteWithWeightInfo,
};
use serde::Deserialize;
use serde_json::Value;
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
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
use tracing::{info, trace, warn};

#[derive(Clone)]
pub enum Options {
    Confirmation(ConfirmationOptions),
    Vote(VoteOptions),
    Other,
}

impl Options {
    /**
     * Checks if a message should be filtered for default options (no options given).
     * @param message_a the message to be checked
     * @return false - the message should always be broadcasted
     */
    pub fn should_filter(&self, message: &Message) -> bool {
        match self {
            Options::Confirmation(i) => i.should_filter(message),
            Options::Vote(i) => i.should_filter(message),
            Options::Other => false,
        }
    }

    /**
     * Update options, if available for a given topic
     * @return false on success
     */
    pub fn update(&mut self, options: &dyn PropertyTree) {
        if let Options::Confirmation(i) = self {
            i.update(options);
        }
    }
}

#[derive(Clone)]
pub struct ConfirmationOptions {
    pub include_election_info: bool,
    pub include_election_info_with_votes: bool,
    pub include_sideband_info: bool,
    pub include_block: bool,
    pub has_account_filtering_options: bool,
    pub all_local_accounts: bool,
    pub confirmation_types: u8,
    pub accounts: HashSet<String>,
    wallets: Arc<Wallets>,
}

#[derive(Deserialize, Default)]
pub struct ConfirmationJsonOptions {
    pub include_block: Option<bool>,
    pub include_election_info: Option<bool>,
    pub include_election_info_with_votes: Option<bool>,
    pub include_sideband_info: Option<bool>,
    pub confirmation_type: Option<String>,
    pub all_local_accounts: Option<bool>,
    pub accounts: Option<Vec<String>>,
}

impl ConfirmationOptions {
    const TYPE_ACTIVE_QUORUM: u8 = 1;
    const TYPE_ACTIVE_CONFIRMATION_HEIGHT: u8 = 2;
    const TYPE_INACTIVE: u8 = 4;
    const TYPE_ALL_ACTIVE: u8 = Self::TYPE_ACTIVE_QUORUM | Self::TYPE_ACTIVE_CONFIRMATION_HEIGHT;
    const TYPE_ALL: u8 = Self::TYPE_ALL_ACTIVE | Self::TYPE_INACTIVE;

    pub fn new(wallets: Arc<Wallets>, options_a: ConfirmationJsonOptions) -> Self {
        let mut result = Self {
            include_election_info: false,
            include_election_info_with_votes: false,
            include_sideband_info: false,
            include_block: true,
            has_account_filtering_options: false,
            all_local_accounts: false,
            confirmation_types: Self::TYPE_ALL,
            accounts: HashSet::new(),
            wallets,
        };
        // Non-account filtering options
        result.include_block = options_a.include_block.unwrap_or(true);
        result.include_election_info = options_a.include_election_info.unwrap_or(false);
        result.include_election_info_with_votes =
            options_a.include_election_info_with_votes.unwrap_or(false);
        result.include_sideband_info = options_a.include_sideband_info.unwrap_or(false);

        let type_l = options_a
            .confirmation_type
            .unwrap_or_else(|| "all".to_string());

        if type_l.eq_ignore_ascii_case("active") {
            result.confirmation_types = Self::TYPE_ALL_ACTIVE;
        } else if type_l.eq_ignore_ascii_case("active_quorum") {
            result.confirmation_types = Self::TYPE_ACTIVE_QUORUM;
        } else if type_l.eq_ignore_ascii_case("active_confirmation_height") {
            result.confirmation_types = Self::TYPE_ACTIVE_CONFIRMATION_HEIGHT;
        } else if type_l.eq_ignore_ascii_case("inactive") {
            result.confirmation_types = Self::TYPE_INACTIVE;
        } else {
            result.confirmation_types = Self::TYPE_ALL;
        }

        // Account filtering options
        let all_local_accounts_l = options_a.all_local_accounts.unwrap_or(false);
        if all_local_accounts_l {
            result.all_local_accounts = true;
            result.has_account_filtering_options = true;
            if !result.include_block {
                warn!("Websocket: Filtering option \"all_local_accounts\" requires that \"include_block\" is set to true to be effective");
            }
        }
        if let Some(accounts_l) = options_a.accounts {
            result.has_account_filtering_options = true;
            for account_l in accounts_l {
                match Account::decode_account(&account_l) {
                    Ok(result_l) => {
                        // Do not insert the given raw data to keep old prefix support
                        result.accounts.insert(result_l.encode_account());
                    }
                    Err(_) => {
                        warn!(
                            "Invalid account provided for filtering blocks: {}",
                            account_l
                        );
                    }
                }
            }

            if !result.include_block {
                warn!("Filtering option \"accounts\" requires that \"include_block\" is set to true to be effective");
            }
        }
        result.check_filter_empty();

        result
    }

    /**
     * Checks if a message should be filtered for given block confirmation options.
     * @param message_a the message to be checked
     * @return false if the message should be broadcasted, true if it should be filtered
     */
    pub fn should_filter(&self, message_a: &Message) -> bool {
        let Some(message_content) = message_a.contents.value.get("message") else {
            return false;
        };
        let mut should_filter_conf_type = true;

        if let Some(serde_json::Value::String(type_text)) = message_content.get("confirmation_type")
        {
            let confirmation_types = self.confirmation_types;
            if type_text == "active_quorum" && (confirmation_types & Self::TYPE_ACTIVE_QUORUM) > 0 {
                should_filter_conf_type = false;
            } else if type_text == "active_confirmation_height"
                && (confirmation_types & Self::TYPE_ACTIVE_CONFIRMATION_HEIGHT) > 0
            {
                should_filter_conf_type = false;
            } else if type_text == "inactive" && (confirmation_types & Self::TYPE_INACTIVE) > 0 {
                should_filter_conf_type = false;
            }
        }

        let mut should_filter_account = self.has_account_filtering_options;
        if let Some(serde_json::Value::Object(block)) = message_content.get("block") {
            if let Some(serde_json::Value::String(destination_text)) = block.get("link_as_account")
            {
                let source_text = match message_content.get("account") {
                    Some(serde_json::Value::String(s)) => s.as_str(),
                    _ => "",
                };
                if self.all_local_accounts {
                    let source = Account::decode_account(source_text).unwrap_or_default();
                    let destination =
                        Account::decode_account(&destination_text).unwrap_or_default();
                    if self.wallets.exists(&source) || self.wallets.exists(&destination) {
                        should_filter_account = false;
                    }
                }
                if self.accounts.contains(source_text) || self.accounts.contains(destination_text) {
                    should_filter_account = false;
                }
            }
        }

        should_filter_conf_type || should_filter_account
    }

    /**
     * Update some existing options
     * Filtering options:
     * - "accounts_add" (array of std::strings) - additional accounts for which blocks should not be filtered
     * - "accounts_del" (array of std::strings) - accounts for which blocks should be filtered
     */
    pub fn update(&mut self, options: &dyn PropertyTree) {
        let mut update_accounts = |accounts_text: &dyn PropertyTree, insert: bool| {
            self.has_account_filtering_options = true;
            for account in accounts_text.get_children() {
                match Account::decode_account(account.1.data()) {
                    Ok(result) => {
                        // Re-encode to keep old prefix support
                        let encoded = result.encode_account();
                        if insert {
                            self.accounts.insert(encoded);
                        } else {
                            self.accounts.remove(&encoded);
                        }
                    }
                    Err(_) => {
                        warn!(
                            "Invalid account provided for filtering blocks: {}",
                            account.1.data()
                        );
                    }
                }
            }
        };

        // Adding accounts as filter exceptions
        if let Some(accounts_add) = options.get_child("accounts_add") {
            update_accounts(&*accounts_add, true);
        }

        // Removing accounts as filter exceptions
        if let Some(accounts_del) = options.get_child("accounts_del") {
            update_accounts(&*accounts_del, false);
        }

        self.check_filter_empty();
    }

    pub fn check_filter_empty(&self) {
        // Warn the user if the options resulted in an empty filter
        if self.has_account_filtering_options
            && !self.all_local_accounts
            && self.accounts.is_empty()
        {
            warn!("Provided options resulted in an empty account confirmation filter");
        }
    }
}

#[derive(Clone)]
pub struct VoteOptions {
    representatives: HashSet<String>,
    include_replays: bool,
    include_indeterminate: bool,
}

#[derive(Deserialize)]
pub struct VoteJsonOptions {
    include_replays: Option<bool>,
    include_indeterminate: Option<bool>,
    representatives: Option<Vec<String>>,
}

impl VoteOptions {
    pub fn new(options_a: VoteJsonOptions) -> Self {
        let mut result = Self {
            representatives: HashSet::new(),
            include_replays: false,
            include_indeterminate: false,
        };

        result.include_replays = options_a.include_replays.unwrap_or(false);
        result.include_indeterminate = options_a.include_indeterminate.unwrap_or(false);
        if let Some(representatives_l) = options_a.representatives {
            for representative_l in representatives_l {
                match Account::decode_account(&representative_l) {
                    Ok(result_l) => {
                        // Do not insert the given raw data to keep old prefix support
                        result.representatives.insert(result_l.encode_account());
                    }
                    Err(_) => {
                        warn!(
                            "Invalid account provided for filtering votes: {}",
                            representative_l
                        );
                    }
                }
            }
            // Warn the user if the option will be ignored
            if result.representatives.is_empty() {
                warn!("Account filter for votes is empty, no messages will be filtered");
            }
        }

        result
    }

    /**
     * Checks if a message should be filtered for given vote received options.
     * @param message_a the message to be checked
     * @return false if the message should be broadcasted, true if it should be filtered
     */
    pub fn should_filter(&self, message_a: &Message) -> bool {
        trace!(
            message = serde_json::to_string_pretty(&message_a.contents.value).unwrap(),
            include_replays = self.include_replays,
            "vote should filter!"
        );
        let Some(message_contents) = message_a.contents.value.get("message") else {
            return true;
        };

        let msg_type = match message_contents.get("type") {
            Some(serde_json::Value::String(s)) => s.as_str(),
            _ => "",
        };

        let mut should_filter_l = (!self.include_replays && msg_type == "replay")
            || (!self.include_indeterminate && msg_type == "indeterminate");

        if !should_filter_l && !self.representatives.is_empty() {
            let representative_text_l = match message_contents.get("account") {
                Some(serde_json::Value::String(s)) => s.as_str(),
                _ => "",
            };

            if !self.representatives.contains(representative_text_l) {
                should_filter_l = true;
            }
        }
        should_filter_l
    }
}

#[derive(Deserialize)]
struct IncomingMessage<'a> {
    action: Option<&'a str>,
    topic: Option<&'a str>,
    #[serde(default)]
    ack: bool,
    id: Option<&'a str>,
    options: Option<Value>,
    #[serde(default)]
    accounts_add: Vec<&'a str>,
    #[serde(default)]
    accounts_del: Vec<&'a str>,
}

struct WebsocketSessionEntry {
    /// Map of subscriptions -> options registered by this session.
    subscriptions: Mutex<HashMap<Topic, Options>>,
    send_queue_tx: mpsc::Sender<Message>,
    close: Mutex<Option<oneshot::Sender<()>>>,
}

impl WebsocketSessionEntry {
    fn blocking_write(&self, msg: Message) -> anyhow::Result<()> {
        if !self.should_filter(&msg) {
            self.send_queue_tx.blocking_send(msg)?;
        }
        Ok(())
    }

    async fn write(&self, msg: Message) -> anyhow::Result<()> {
        if !self.should_filter(&msg) {
            self.send_queue_tx.send(msg).await?
        }
        Ok(())
    }

    fn should_filter(&self, msg: &Message) -> bool {
        if msg.topic == Topic::Ack {
            return false;
        }

        let subs = self.subscriptions.lock().unwrap();
        if let Some(options) = subs.get(&msg.topic) {
            options.should_filter(&msg)
        } else {
            true
        }
    }
}

pub struct WebsocketSession {
    entry: Arc<WebsocketSessionEntry>,
    wallets: Arc<Wallets>,
    topic_subscriber_count: Arc<[AtomicUsize; 11]>,
    remote_endpoint: SocketAddr,
}

impl WebsocketSession {
    fn new(
        wallets: Arc<Wallets>,
        topic_subscriber_count: Arc<[AtomicUsize; 11]>,
        remote_endpoint: SocketAddr,
        entry: Arc<WebsocketSessionEntry>,
    ) -> Self {
        trace!(remote = %remote_endpoint, "new websocket session created");
        Self {
            entry,
            wallets,
            topic_subscriber_count,
            remote_endpoint,
        }
    }

    pub async fn run(
        self,
        stream: &mut tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
        send_queue: &mut mpsc::Receiver<Message>,
    ) -> anyhow::Result<()> {
        loop {
            tokio::select! {
                Some(msg) = stream.next() =>{
                    if !self.process(msg?).await {
                        break;
                    }
                }
                Some(msg) = send_queue.recv() =>{
                    let message_text = msg.contents.to_json();
                    trace!(message = message_text, "sending websocket message");
                    // write queued messages
                    stream
                        .send(tokio_tungstenite::tungstenite::Message::text(
                            message_text,
                        )).await?;
                }
                else =>{
                    break;
                }
            }
        }
        Ok(())
    }

    async fn process(&self, msg: tokio_tungstenite::tungstenite::Message) -> bool {
        if msg.is_close() {
            trace!("close message received");
            false
        } else if msg.is_text() {
            let msg_text = match msg.into_text() {
                Ok(i) => i,
                Err(e) => {
                    warn!("Could not deserialize string: {:?}", e);
                    return false;
                }
            };

            trace!(message = msg_text, "Received text websocket message");

            let incoming = match serde_json::from_str::<IncomingMessage>(&msg_text) {
                Ok(i) => i,
                Err(e) => {
                    warn!(
                        text = msg_text,
                        "Could not deserialize JSON message: {:?}", e
                    );
                    return false;
                }
            };

            if let Err(e) = self.handle_message(incoming).await {
                warn!("Could not process websocket message: {:?}", e);
                return false;
            }
            true
        } else {
            true
        }
    }

    async fn handle_message(&self, message: IncomingMessage<'_>) -> anyhow::Result<()> {
        let topic = to_topic(message.topic.unwrap_or(""));
        let mut action_succeeded = false;
        let mut ack = message.ack;
        let mut reply_action = message.action.unwrap_or("");
        if message.action == Some("subscribe") && topic != Topic::Invalid {
            let mut subs = self.entry.subscriptions.lock().unwrap();
            let options = match topic {
                Topic::Confirmation => {
                    if let Some(options_value) = message.options {
                        Options::Confirmation(ConfirmationOptions::new(
                            Arc::clone(&self.wallets),
                            serde_json::from_value::<ConfirmationJsonOptions>(options_value)?,
                        ))
                    } else {
                        Options::Other
                    }
                }
                Topic::Vote => {
                    if let Some(options_value) = message.options {
                        Options::Vote(VoteOptions::new(serde_json::from_value::<VoteJsonOptions>(
                            options_value,
                        )?))
                    } else {
                        Options::Other
                    }
                }
                _ => Options::Other,
            };
            let inserted = subs.insert(topic, options).is_none();
            if inserted {
                self.topic_subscriber_count[topic as usize].fetch_add(1, Ordering::SeqCst);
            }
            action_succeeded = true;
        } else if message.action == Some("update") {
            let mut subs = self.entry.subscriptions.lock().unwrap();
            if let Some(option) = subs.get_mut(&topic) {
                if let Some(options_value) = message.options {
                    option.update(&SerdePropertyTree::from_value(options_value));
                    action_succeeded = true;
                }
            }
        } else if message.action == Some("unsubscribe") && topic != Topic::Invalid {
            let mut subs = self.entry.subscriptions.lock().unwrap();
            if subs.remove(&topic).is_some() {
                info!(
                    "Removed subscription to topic: {} ({})",
                    topic.as_str(),
                    self.remote_endpoint
                );
                self.topic_subscriber_count[topic as usize].fetch_sub(1, Ordering::SeqCst);
            }
            action_succeeded = true;
        } else if message.action == Some("ping") {
            action_succeeded = true;
            ack = true;
            reply_action = "pong";
        }
        if ack && action_succeeded {
            self.send_ack(reply_action, &message.id).await?;
        }
        Ok(())
    }

    async fn send_ack(&self, reply_action: &str, id: &Option<&str>) -> anyhow::Result<()> {
        let mut vals = serde_json::Map::new();
        vals.insert("ack".to_string(), Value::String(reply_action.to_string()));
        vals.insert(
            "time".to_string(),
            Value::String(milliseconds_since_epoch().to_string()),
        );
        if let Some(id) = id {
            vals.insert("id".to_string(), Value::String(id.to_string()));
        }
        let contents = serde_json::Value::Object(vals);
        let msg = Message {
            topic: Topic::Ack,
            contents: SerdePropertyTree::from_value(contents),
        };

        self.entry.write(msg).await
    }
}

impl Drop for WebsocketSession {
    fn drop(&mut self) {
        trace!(remote = %self.remote_endpoint, "websocket session dropped");
        let subs = self.entry.subscriptions.lock().unwrap();
        for (topic, _) in subs.iter() {
            self.topic_subscriber_count[*topic as usize].fetch_sub(1, Ordering::SeqCst);
        }
    }
}

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

    pub fn subscriber_count(&self, topic: Topic) -> usize {
        self.topic_subscriber_count[topic as usize].load(Ordering::SeqCst)
    }

    async fn run2(&self) {
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
    pub async fn stop2(&self) {
        if let Some(tx) = self.tx_stop.lock().unwrap().take() {
            tx.send(()).unwrap()
        }

        let mut sessions = self.sessions.lock().unwrap();
        for session in sessions.drain(..) {
            if let Some(session) = session.upgrade() {
                if let Some(close) = session.close.lock().unwrap().take() {
                    let _ = close.send(());
                }
            }
        }
    }

    pub fn listening_port(&self) -> u16 {
        self.endpoint.lock().unwrap().port()
    }

    /// Broadcast \p message to all session subscribing to the message topic.
    pub fn broadcast(&self, message: &Message) -> anyhow::Result<()> {
        let sessions = self.sessions.lock().unwrap();
        for session in sessions.iter() {
            if let Some(session) = session.upgrade() {
                let _ = session.blocking_write(message.clone());
            }
        }

        Ok(())
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
    fn run(&self);
    fn stop(&self);
}

impl WebsocketListenerExt for Arc<WebsocketListener> {
    /// Start accepting connections
    fn run(&self) {
        let self_l = Arc::clone(self);
        self.async_rt.tokio.spawn(async move {
            self_l.run2().await;
        });
    }

    fn stop(&self) {
        let self_l = Arc::clone(self);
        self.async_rt.tokio.spawn(async move {
            self_l.stop2().await;
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

    let (close_tx, close_rx) = oneshot::channel::<()>();
    let entry = Arc::new(WebsocketSessionEntry {
        subscriptions: Mutex::new(HashMap::new()),
        send_queue_tx: tx_send,
        close: Mutex::new(Some(close_tx)),
    });

    {
        let mut sessions = sessions.lock().unwrap();
        sessions.retain(|s| s.strong_count() > 0);
        sessions.push(Arc::downgrade(&entry));
    }

    let session = WebsocketSession::new(wallets, topic_subscriber_count, remote_endpoint, entry);

    tokio::select! {
        _ = close_rx =>{
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
