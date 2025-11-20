use std::{
    ffi::OsString,
    net::{Ipv6Addr, SocketAddrV6},
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    thread::yield_now,
    time::{Duration, Instant},
};

use clap::Parser;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf},
    select,
    sync::mpsc,
    task::JoinSet,
};
use tokio_util::sync::CancellationToken;
use tracing::info;

use rsnano_core::{Block, Networks, PrivateKey, ProtocolInfo, RawKey};
use rsnano_messages::{Message, MessageSerializer, Publish};
use rsnano_network::token_bucket::TokenBucket;
use rsnano_nullable_clock::SteadyClock;
use rsnano_nullable_tcp::{TcpStream, TcpStreamFactory};
use rsnano_nullable_tracing_subscriber::TracingInitializer;
use rsnano_rpc_client::NanoRpcClient;
use rsnano_websocket_messages::MessageEnvelope;

use crate::{
    confirmation_receiver::ConfirmationReceiver,
    confirmation_tracker::track_confirmations,
    domain::{BlockFactory, BlockResult, DelayedBlocks, RateSpec, SpamStrategy},
    frontiers_sync::sync_frontiers,
    handshake::perform_handshake,
    high_prio_check::{HighPrioCheck, HighPrioTracker},
    setup::{
        configure_nodes, create_account_map, get_genesis_hash, peering_port, rpc_port, start_nodes,
    },
    wallets_factory::create_wallets,
};

const MAX_BUFFERED_BLOCKS: usize = 1024;
const DEFAULT_RATE: &str = "1+50@3s";
const CONNECTIONS_PER_NODE: usize = 4;

#[derive(Parser, Debug)]
pub(crate) struct Args {
    /// Number of principal representatives
    #[arg(long, default_value_t = 1)]
    pub prs: usize,

    /// Only create the node config files and set up the wallets, then exit
    #[arg(long, default_value_t = false)]
    pub setup_only: bool,

    /// Attach to an already running node
    #[arg(long, default_value_t = false)]
    pub attach: bool,

    #[arg(long)]
    /// Block rate in the form "1000+50@3s" or "1000"
    pub rate: Option<String>,

    #[arg(long)]
    /// Number of blocks to publish
    pub blocks: Option<usize>,

    /// Don't wait for a block to get confirmed before publishing the next block
    #[arg(long, default_value_t = false)]
    pub unconfirmed: bool,

    /// Query frontiers of the spam accounts before starting spam
    #[arg(long, default_value_t = false)]
    pub sync: bool,

    /// Only publish change blocks. This requires --sync
    #[arg(long, default_value_t = false)]
    pub change: bool,

    /// Run the C++ nano_node (must be in $PATH)
    #[arg(long, default_value_t = false)]
    pub cpp: bool,

    /// Use RocksDB (works only for nano_node)
    #[arg(long, default_value_t = false)]
    pub rocksdb: bool,

    /// Disable sending a high priority block every 10s
    #[arg(long, default_value_t = false)]
    pub no_prio: bool,

    /// Limit confirmations per second
    #[arg(long, default_value_t = 0)]
    pub cps_limit: u32,

    /// Don't kill the node processes on exit
    #[arg(long, default_value_t = false)]
    pub no_kill: bool,

    /// Don't republish delayed blocks after 10 seconds
    #[arg(long, default_value_t = false)]
    pub no_republish: bool,

    /// Maximum number of individual accounts to use to produce blocks
    #[arg(long, default_value_t = 500000)]
    pub accounts: usize,
}

#[derive(Default)]
pub(crate) struct NanoSpamApp {
    pub tracing_init: TracingInitializer,
    pub tcp_stream_factory: TcpStreamFactory,
}

impl NanoSpamApp {
    pub async fn run<I, T>(&self, args: I) -> anyhow::Result<()>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        self.tracing_init.init();
        let args = Args::try_parse_from(args)?;

        let protocol = ProtocolInfo::default_for(Networks::NanoTestNetwork);
        let genesis_hash = get_genesis_hash();
        let rate_spec: RateSpec = args
            .rate
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or(DEFAULT_RATE)
            .parse()?;

        let mut data_dir = dirs::home_dir().unwrap();
        data_dir.push("NanoSpam");

        let mut account_map = create_account_map(&data_dir, args.accounts);

        if !args.attach && !args.sync {
            configure_nodes(&args, &data_dir);
        }

        let mut rpc_clients = Vec::new();
        for i in 0..args.prs {
            let rpc_client =
                NanoRpcClient::new(format!("http://[::1]:{}", rpc_port(i)).parse().unwrap());
            rpc_clients.push(rpc_client);
        }
        let genesis_rpc = &rpc_clients[0];
        let delayed_blocks = Mutex::new(DelayedBlocks::new());

        let mut node_handles = Vec::new();

        if !args.attach {
            node_handles = start_nodes(&args, data_dir, &rpc_clients).await
        }

        let (tx_block, rx_block) = mpsc::channel::<Block>(MAX_BUFFERED_BLOCKS);
        let high_prio_tracker = Mutex::new(HighPrioTracker::default());
        let mut high_prio_check =
            HighPrioCheck::new(genesis_rpc, &delayed_blocks, &high_prio_tracker);

        if !args.attach && !args.sync {
            let genesis_wallet_id =
                create_wallets(&args, &rpc_clients, genesis_rpc, &mut account_map).await;
            high_prio_check
                .create_prio_accounts(genesis_wallet_id)
                .await?;
        }

        if args.setup_only {
            return Ok(());
        }

        if args.sync {
            sync_frontiers(&rpc_clients, &mut account_map).await;
            high_prio_check.sync_accounts().await?;
        }

        let mut tcp_writers = Vec::new();
        let mut tcp_readers = Vec::new();

        for node_index in 0..args.prs {
            let peer_addr = SocketAddrV6::new(Ipv6Addr::LOCALHOST, peering_port(node_index), 0, 0);
            info!(?peer_addr, "Connecting to node PR{node_index}...");
            let mut node_writers = Vec::with_capacity(CONNECTIONS_PER_NODE);
            let mut node_readers = Vec::with_capacity(CONNECTIONS_PER_NODE);
            for i in 0..CONNECTIONS_PER_NODE {
                let mut tcp_stream = self.tcp_stream_factory.connect(peer_addr).await?;
                info!("Performing handshake...");
                let node_id_key: PrivateKey = RawKey::from(42 + i as u64).into();
                perform_handshake(protocol, genesis_hash, node_id_key, &mut tcp_stream).await?;
                let (tcp_read, tcp_write) = tokio::io::split(tcp_stream);
                node_writers.push(tcp_write);
                node_readers.push(tcp_read);
            }
            tcp_writers.push(node_writers);
            tcp_readers.push(node_readers);
        }

        let tx_block_clone = tx_block.clone();
        let cancel_block_creation = CancellationToken::new();
        let cancel_tcp_recv = CancellationToken::new();
        let cancel_ws_recv = CancellationToken::new();
        let current_bps = AtomicUsize::new(rate_spec.initial_bps);

        let ws_queue_len = AtomicUsize::new(0);
        let (tx_ws_msg, rx_ws_msg) = std::sync::mpsc::channel::<(MessageEnvelope, Instant)>();
        let mut sum_conf_time = Duration::ZERO;

        let strategy = if args.change {
            SpamStrategy::Change
        } else {
            SpamStrategy::SendReceive
        };

        let block_factory = Mutex::new(BlockFactory::new(
            account_map,
            args.blocks.unwrap_or(0),
            strategy,
        ));

        info!("Connecting to websocket...");
        let mut conf_receiver = ConfirmationReceiver::connect().await?;
        info!("Starting with {} BPS", current_bps.load(Ordering::Relaxed));
        let started = Instant::now();
        std::thread::scope(|s| {
            s.spawn(|| {
                track_confirmations(
                    rx_ws_msg,
                    &delayed_blocks,
                    &block_factory,
                    &ws_queue_len,
                    &mut sum_conf_time,
                    &current_bps,
                    !args.unconfirmed,
                    &high_prio_tracker,
                )
            });

            let cancel_blk = cancel_block_creation.clone();
            s.spawn(|| {
                create_blocks(
                    &block_factory,
                    tx_block,
                    &delayed_blocks,
                    &current_bps,
                    rate_spec,
                    cancel_blk,
                )
            });

            tokio_scoped::scope(|scope| {
                if !args.no_prio {
                    scope.spawn(high_prio_check.run(cancel_block_creation, tx_block_clone.clone()));
                }
                scope.spawn(conf_receiver.run(cancel_ws_recv.clone(), &ws_queue_len, tx_ws_msg));
                scope.spawn(receive_messages(
                    tcp_readers,
                    protocol,
                    cancel_tcp_recv.clone(),
                ));
                if !args.no_republish {
                    scope.spawn(republish_delayed_blocks(
                        tx_block_clone,
                        &delayed_blocks,
                        cancel_ws_recv,
                    ));
                }
                scope.spawn(publish_blocks(
                    rx_block,
                    tcp_writers,
                    protocol,
                    &delayed_blocks,
                    cancel_tcp_recv,
                    args.unconfirmed,
                    &block_factory,
                    &high_prio_tracker,
                ));
            });
        });
        let duration_secs = started.elapsed().as_secs_f64();
        let created_blocks = block_factory.lock().unwrap().created();
        let cps = (created_blocks as f64 / duration_secs) as i32;
        info!("Confirming all blocks took {duration_secs:.2}s");
        info!("Confirmation rate: {cps} cps");
        let conf_time = sum_conf_time.as_millis() / created_blocks as u128;
        info!("Average conf time: {conf_time} ms");

        if !args.no_kill {
            for mut child in node_handles {
                child.kill().unwrap();
            }
        }
        Ok(())
    }
}

fn create_blocks(
    block_factory: &Mutex<BlockFactory>,
    tx_block: mpsc::Sender<Block>,
    delayed_blocks: &Mutex<DelayedBlocks>,
    current_bps: &AtomicUsize,
    rate_spec: RateSpec,
    cancel_token: CancellationToken,
) {
    let mut bps_start = Instant::now();
    let mut limiter = TokenBucket::new(current_bps.load(Ordering::Relaxed));
    let clock = SteadyClock::default();

    while let Some(result) = {
        let mut guard = block_factory.lock().unwrap();
        guard.create_next()
    } {
        let BlockResult::Block(block) = result else {
            yield_now();
            continue;
        };

        while !limiter.try_consume(1, clock.now()) {
            std::thread::yield_now();
        }

        {
            let mut delayed = delayed_blocks.lock().unwrap();
            delayed.insert(block.clone());
        }

        tx_block.blocking_send(block).unwrap();
        if bps_start.elapsed() >= rate_spec.interval {
            let new_bps =
                current_bps.fetch_add(rate_spec.increment, Ordering::Relaxed) + rate_spec.increment;
            limiter.set_limit(new_bps);
            bps_start = Instant::now();
        }
    }
    delayed_blocks.lock().unwrap().finished();
    cancel_token.cancel();
}

async fn publish_blocks(
    mut rx_block: mpsc::Receiver<Block>,
    mut tcp_streams: Vec<Vec<WriteHalf<TcpStream>>>,
    protocol: ProtocolInfo,
    delayed_blocks: &Mutex<DelayedBlocks>,
    cancel_token: CancellationToken,
    unconfirmed: bool,
    block_factory: &Mutex<BlockFactory>,
    prio_tracker: &Mutex<HighPrioTracker>,
) {
    let mut serializer = MessageSerializer::new(protocol);
    let mut writer_index = 0;
    while let Some(block) = rx_block.recv().await {
        let hash = block.hash();
        let publish = Message::Publish(Publish::new_from_originator(block));
        let buffer = serializer.serialize(&publish);

        let now = Instant::now();
        delayed_blocks.lock().unwrap().published(&hash, now);

        tokio_scoped::scope(|s| {
            for stream in &mut tcp_streams {
                s.spawn(async {
                    stream[writer_index].write(buffer).await.unwrap();
                });
            }
        });

        writer_index += 1;
        if writer_index >= CONNECTIONS_PER_NODE {
            writer_index = 0;
        }

        if unconfirmed {
            delayed_blocks.lock().unwrap().confirmed(&hash, now);
            block_factory.lock().unwrap().confirm(hash);
        }
        prio_tracker.lock().unwrap().published(hash);
    }
    cancel_token.cancel();
}

async fn republish_delayed_blocks(
    tx_block: mpsc::Sender<Block>,
    delayed_blocks: &Mutex<DelayedBlocks>,
    cancel_token: CancellationToken,
) {
    loop {
        while let Some(block) = {
            let now = Instant::now();
            delayed_blocks.lock().unwrap().next(now)
        } {
            tx_block.send(block).await.unwrap();
        }

        if delayed_blocks.lock().unwrap().is_finished() {
            break;
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    cancel_token.cancel();
}

async fn receive_messages(
    mut readers: Vec<Vec<ReadHalf<TcpStream>>>,
    _protocol: ProtocolInfo,
    cancel_token: CancellationToken,
) {
    select! {
        _ = cancel_token.cancelled() => {},
        _ = async {
            let mut set = JoinSet::new();
            for mut reader in readers.drain(..).flatten() {
                set.spawn(async move {
                    let mut recv_buffer = vec![0; 1024 * 4];
                    loop{
                        reader.read(&mut recv_buffer).await.unwrap();
                    }
                });
            }
            set.join_all().await;
        } => {}
    }
}
