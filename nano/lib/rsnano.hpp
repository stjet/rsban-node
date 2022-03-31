#ifndef rs_nano_bindings_hpp
#define rs_nano_bindings_hpp

/* Warning, this file is autogenerated by cbindgen. Don't modify this manually. */

#include <cstdarg>
#include <cstdint>
#include <cstdlib>
#include <new>
#include <ostream>

namespace rsnano
{

static const uintptr_t SignatureChecker_BATCH_SIZE = 256;

struct BandwidthLimiterHandle;

struct ChangeBlockHandle;

struct OpenBlockHandle;

struct ReceiveBlockHandle;

struct SendBlockHandle;

struct SignatureCheckerHandle;

struct StateBlockHandle;

struct BlockDetailsDto
{
	uint8_t epoch;
	bool is_send;
	bool is_receive;
	bool is_epoch;
};

struct BlockDto
{
	uint8_t block_type;
	void * handle;
};

struct BlockSidebandDto
{
	uint64_t height;
	uint64_t timestamp;
	uint8_t successor[32];
	uint8_t account[32];
	uint8_t balance[16];
	BlockDetailsDto details;
	uint8_t source_epoch;
};

struct WorkThresholdsDto
{
	uint64_t epoch_1;
	uint64_t epoch_2;
	uint64_t epoch_2_receive;
	uint64_t base;
	uint64_t entry;
};

struct NetworkConstantsDto
{
	uint16_t current_network;
	WorkThresholdsDto work;
	uint32_t principal_weight_factor;
	uint16_t default_node_port;
	uint16_t default_rpc_port;
	uint16_t default_ipc_port;
	uint16_t default_websocket_port;
	uint32_t request_interval_ms;
	int64_t cleanup_period_s;
	int64_t idle_timeout_s;
	int64_t sync_cookie_cutoff_s;
	int64_t bootstrap_interval_s;
	uintptr_t max_peers_per_ip;
	uintptr_t max_peers_per_subnetwork;
	int64_t peer_dump_interval_s;
	uint8_t protocol_version;
	uint8_t protocol_version_min;
	uintptr_t ipv6_subnetwork_prefix_for_limiting;
	int64_t silent_connection_tolerance_time_s;
};

struct BootstrapConstantsDto
{
	uint32_t lazy_max_pull_blocks;
	uint32_t lazy_min_pull_blocks;
	uint32_t frontier_retry_limit;
	uint32_t lazy_retry_limit;
	uint32_t lazy_destinations_retry_limit;
	int64_t gap_cache_bootstrap_start_interval_ms;
	uint32_t default_frontiers_age_seconds;
};

using Blake2BFinalCallback = int32_t (*) (void *, void *, uintptr_t);

using Blake2BInitCallback = int32_t (*) (void *, uintptr_t);

using Blake2BUpdateCallback = int32_t (*) (void *, const void *, uintptr_t);

using PropertyTreeGetStringCallback = int32_t (*) (const void *, const char *, uintptr_t, char *, uintptr_t);

using PropertyTreePutStringCallback = void (*) (void *, const char *, uintptr_t, const char *, uintptr_t);

using ReadBytesCallback = int32_t (*) (void *, uint8_t *, uintptr_t);

using ReadU8Callback = int32_t (*) (void *, uint8_t *);

using TomlArrayPutStrCallback = void (*) (void *, const uint8_t *, uintptr_t);

using TomlCreateArrayCallback = void * (*)(void *, const uint8_t *, uintptr_t, const uint8_t *, uintptr_t);

using TomlCreateConfigCallback = void * (*)();

using TomlDropArrayCallback = void (*) (void *);

using TomlDropConfigCallback = void (*) (void *);

using TomlPutBoolCallback = int32_t (*) (void *, const uint8_t *, uintptr_t, bool, const uint8_t *, uintptr_t);

using TomlPutChildCallback = void (*) (void *, const uint8_t *, uintptr_t, void *);

using TomlPutF64Callback = int32_t (*) (void *, const uint8_t *, uintptr_t, double, const uint8_t *, uintptr_t);

using TomlPutI64Callback = int32_t (*) (void *, const uint8_t *, uintptr_t, int64_t, const uint8_t *, uintptr_t);

using TomlPutStrCallback = int32_t (*) (void *, const uint8_t *, uintptr_t, const uint8_t *, uintptr_t, const uint8_t *, uintptr_t);

using TomlPutU64Callback = int32_t (*) (void *, const uint8_t *, uintptr_t, uint64_t, const uint8_t *, uintptr_t);

using WriteBytesCallback = int32_t (*) (void *, const uint8_t *, uintptr_t);

using WriteU8Callback = int32_t (*) (void *, uint8_t);

struct ChangeBlockDto
{
	uint64_t work;
	uint8_t signature[64];
	uint8_t previous[32];
	uint8_t representative[32];
};

struct ChangeBlockDto2
{
	uint8_t previous[32];
	uint8_t representative[32];
	uint8_t priv_key[32];
	uint8_t pub_key[32];
	uint64_t work;
};

struct PeerDto
{
	uint8_t address[128];
	uintptr_t address_len;
	uint16_t port;
};

struct LoggingDto
{
	bool ledger_logging_value;
	bool ledger_duplicate_logging_value;
	bool ledger_rollback_logging_value;
	bool vote_logging_value;
	bool rep_crawler_logging_value;
	bool election_fork_tally_logging_value;
	bool election_expiration_tally_logging_value;
	bool network_logging_value;
	bool network_timeout_logging_value;
	bool network_message_logging_value;
	bool network_publish_logging_value;
	bool network_packet_logging_value;
	bool network_keepalive_logging_value;
	bool network_node_id_handshake_logging_value;
	bool network_telemetry_logging_value;
	bool network_rejected_logging_value;
	bool node_lifetime_tracing_value;
	bool insufficient_work_logging_value;
	bool log_ipc_value;
	bool bulk_pull_logging_value;
	bool work_generation_time_value;
	bool upnp_details_logging_value;
	bool timing_logging_value;
	bool active_update_value;
	bool log_to_cerr_value;
	bool flush;
	uintptr_t max_size;
	uintptr_t rotation_size;
	bool stable_log_filename;
	int64_t min_time_between_log_output_ms;
	bool single_line_record_value;
	bool election_result_logging_value;
};

struct WebsocketConfigDto
{
	bool enabled;
	uint16_t port;
	uint8_t address[128];
	uintptr_t address_len;
};

struct IpcConfigTransportDto
{
	bool enabled;
	bool allow_unsafe;
	uintptr_t io_timeout;
	int64_t io_threads;
};

struct IpcConfigDto
{
	IpcConfigTransportDto domain_transport;
	uint8_t domain_path[512];
	uintptr_t domain_path_len;
	IpcConfigTransportDto tcp_transport;
	NetworkConstantsDto tcp_network_constants;
	uint16_t tcp_port;
	bool flatbuffers_skip_unexpected_fields_in_json;
	bool flatbuffers_verify_buffers;
};

struct TxnTrackingConfigDto
{
	bool enable;
	int64_t min_read_txn_time_ms;
	int64_t min_write_txn_time_ms;
	bool ignore_writes_below_block_processor_max_time;
};

struct StatConfigDto
{
	bool sampling_enabled;
	uintptr_t capacity;
	uintptr_t interval;
	uintptr_t log_interval_samples;
	uintptr_t log_interval_counters;
	uintptr_t log_rotation_count;
	bool log_headers;
	uint8_t log_counters_filename[128];
	uintptr_t log_counters_filename_len;
	uint8_t log_samples_filename[128];
	uintptr_t log_samples_filename_len;
};

struct RocksDbConfigDto
{
	bool enable;
	uint8_t memory_multiplier;
	uint32_t io_threads;
};

struct LmdbConfigDto
{
	uint8_t sync;
	uint32_t max_databases;
	uintptr_t map_size;
};

struct NodeConfigDto
{
	uint16_t peering_port;
	bool peering_port_defined;
	uint32_t bootstrap_fraction_numerator;
	uint8_t receive_minimum[16];
	uint8_t online_weight_minimum[16];
	uint32_t election_hint_weight_percent;
	uint32_t password_fanout;
	uint32_t io_threads;
	uint32_t network_threads;
	uint32_t work_threads;
	uint32_t signature_checker_threads;
	bool enable_voting;
	uint32_t bootstrap_connections;
	uint32_t bootstrap_connections_max;
	uint32_t bootstrap_initiator_threads;
	uint32_t bootstrap_frontier_request_count;
	int64_t block_processor_batch_max_time_ms;
	bool allow_local_peers;
	uint8_t vote_minimum[16];
	int64_t vote_generator_delay_ms;
	uint32_t vote_generator_threshold;
	int64_t unchecked_cutoff_time_s;
	int64_t tcp_io_timeout_s;
	int64_t pow_sleep_interval_ns;
	uint8_t external_address[128];
	uintptr_t external_address_len;
	uint16_t external_port;
	uint32_t tcp_incoming_connections_max;
	bool use_memory_pools;
	uintptr_t confirmation_history_size;
	uintptr_t active_elections_size;
	uintptr_t bandwidth_limit;
	double bandwidth_limit_burst_ratio;
	int64_t conf_height_processor_batch_min_time_ms;
	bool backup_before_upgrade;
	double max_work_generate_multiplier;
	uint8_t frontiers_confirmation;
	uint32_t max_queued_requests;
	uint32_t confirm_req_batches_max;
	uint8_t rep_crawler_weight_minimum[16];
	PeerDto work_peers[5];
	uintptr_t work_peers_count;
	PeerDto secondary_work_peers[5];
	uintptr_t secondary_work_peers_count;
	PeerDto preconfigured_peers[5];
	uintptr_t preconfigured_peers_count;
	uint8_t preconfigured_representatives[10][32];
	uintptr_t preconfigured_representatives_count;
	int64_t max_pruning_age_s;
	uint64_t max_pruning_depth;
	uint8_t callback_address[128];
	uintptr_t callback_address_len;
	uint16_t callback_port;
	uint8_t callback_target[128];
	uintptr_t callback_target_len;
	LoggingDto logging;
	WebsocketConfigDto websocket_config;
	IpcConfigDto ipc_config;
	TxnTrackingConfigDto diagnostics_config;
	StatConfigDto stat_config;
	RocksDbConfigDto rocksdb_config;
	LmdbConfigDto lmdb_config;
};

struct OpenclConfigDto
{
	uint32_t platform;
	uint32_t device;
	uint32_t threads;
};

struct NodePowServerConfigDto
{
	bool enable;
	uint8_t pow_server_path[128];
	uintptr_t pow_server_path_len;
};

struct NodeRpcConfigDto
{
	uint8_t rpc_path[512];
	uintptr_t rpc_path_length;
	bool enable_child_process;
	bool enable_sign_hash;
};

struct DaemonConfigDto
{
	bool rpc_enable;
	NodeConfigDto node;
	OpenclConfigDto opencl;
	bool opencl_enable;
	NodePowServerConfigDto pow_server;
	NodeRpcConfigDto rpc;
};

struct LedgerConstantsDto
{
	WorkThresholdsDto work;
	uint8_t priv_key[32];
	uint8_t pub_key[32];
	uint8_t nano_beta_account[32];
	uint8_t nano_live_account[32];
	uint8_t nano_test_account[32];
	BlockDto nano_dev_genesis;
	BlockDto nano_beta_genesis;
	BlockDto nano_live_genesis;
	BlockDto nano_test_genesis;
	BlockDto genesis;
	uint8_t genesis_amount[16];
	uint8_t burn_account[32];
	uint8_t nano_dev_final_votes_canary_account[32];
	uint8_t nano_beta_final_votes_canary_account[32];
	uint8_t nano_live_final_votes_canary_account[32];
	uint8_t nano_test_final_votes_canary_account[32];
	uint8_t final_votes_canary_account[32];
	uint64_t nano_dev_final_votes_canary_height;
	uint64_t nano_beta_final_votes_canary_height;
	uint64_t nano_live_final_votes_canary_height;
	uint64_t nano_test_final_votes_canary_height;
	uint64_t final_votes_canary_height;
	uint8_t epoch_1_signer[32];
	uint8_t epoch_1_link[32];
	uint8_t epoch_2_signer[32];
	uint8_t epoch_2_link[32];
};

struct VotingConstantsDto
{
	uintptr_t max_cache;
	int64_t delay_s;
};

struct NodeConstantsDto
{
	int64_t backup_interval_m;
	int64_t search_pending_interval_s;
	int64_t unchecked_cleaning_interval_m;
	int64_t process_confirmed_interval_ms;
	uint64_t max_weight_samples;
	uint64_t weight_period;
};

struct PortmappingConstantsDto
{
	int64_t lease_duration_s;
	int64_t health_check_period_s;
};

struct NetworkParamsDto
{
	uint32_t kdf_work;
	WorkThresholdsDto work;
	NetworkConstantsDto network;
	LedgerConstantsDto ledger;
	VotingConstantsDto voting;
	NodeConstantsDto node;
	PortmappingConstantsDto portmapping;
	BootstrapConstantsDto bootstrap;
};

struct OpenBlockDto
{
	uint64_t work;
	uint8_t signature[64];
	uint8_t source[32];
	uint8_t representative[32];
	uint8_t account[32];
};

struct OpenBlockDto2
{
	uint8_t source[32];
	uint8_t representative[32];
	uint8_t account[32];
	uint8_t priv_key[32];
	uint8_t pub_key[32];
	uint64_t work;
};

struct ReceiveBlockDto
{
	uint64_t work;
	uint8_t signature[64];
	uint8_t previous[32];
	uint8_t source[32];
};

struct ReceiveBlockDto2
{
	uint8_t previous[32];
	uint8_t source[32];
	uint8_t priv_key[32];
	uint8_t pub_key[32];
	uint64_t work;
};

struct RpcProcessConfigDto
{
	uint32_t io_threads;
	uint8_t ipc_address[128];
	uintptr_t ipc_address_len;
	uint16_t ipc_port;
	uint32_t num_ipc_connections;
};

struct RpcConfigDto
{
	uint8_t address[128];
	uintptr_t address_len;
	uint16_t port;
	bool enable_control;
	uint8_t max_json_depth;
	uint64_t max_request_size;
	bool rpc_log;
	RpcProcessConfigDto rpc_process;
};

struct SendBlockDto
{
	uint8_t previous[32];
	uint8_t destination[32];
	uint8_t balance[16];
	uint8_t signature[64];
	uint64_t work;
};

struct SendBlockDto2
{
	uint8_t previous[32];
	uint8_t destination[32];
	uint8_t balance[16];
	uint8_t priv_key[32];
	uint8_t pub_key[32];
	uint64_t work;
};

struct SignatureCheckSetDto
{
	uintptr_t size;
	const uint8_t * const * messages;
	const uintptr_t * message_lengths;
	const uint8_t * const * pub_keys;
	const uint8_t * const * signatures;
	int32_t * verifications;
};

struct StateBlockDto
{
	uint8_t signature[64];
	uint8_t account[32];
	uint8_t previous[32];
	uint8_t representative[32];
	uint8_t link[32];
	uint8_t balance[16];
	uint64_t work;
};

struct StateBlockDto2
{
	uint8_t account[32];
	uint8_t previous[32];
	uint8_t representative[32];
	uint8_t link[32];
	uint8_t balance[16];
	uint8_t priv_key[32];
	uint8_t pub_key[32];
	uint64_t work;
};

extern "C" {

int32_t rsn_account_decode (const char * input, uint8_t (*result)[32]);

void rsn_account_encode (const uint8_t (*bytes)[32], uint8_t (*result)[65]);

BandwidthLimiterHandle * rsn_bandwidth_limiter_create (double limit_burst_ratio, uintptr_t limit);

void rsn_bandwidth_limiter_destroy (BandwidthLimiterHandle * limiter);

int32_t rsn_bandwidth_limiter_reset (const BandwidthLimiterHandle * limiter,
double limit_burst_ratio,
uintptr_t limit);

bool rsn_bandwidth_limiter_should_drop (const BandwidthLimiterHandle * limiter,
uintptr_t message_size,
int32_t * result);

int32_t rsn_block_details_create (uint8_t epoch,
bool is_send,
bool is_receive,
bool is_epoch,
BlockDetailsDto * result);

int32_t rsn_block_details_deserialize (BlockDetailsDto * dto, void * stream);

int32_t rsn_block_details_serialize (const BlockDetailsDto * dto, void * stream);

bool rsn_block_has_sideband (const BlockDto * block);

uintptr_t rsn_block_serialized_size (uint8_t block_type);

int32_t rsn_block_sideband (const BlockDto * block, BlockSidebandDto * sideband);

int32_t rsn_block_sideband_deserialize (BlockSidebandDto * dto, void * stream, uint8_t block_type);

int32_t rsn_block_sideband_serialize (const BlockSidebandDto * dto, void * stream, uint8_t block_type);

int32_t rsn_block_sideband_set (BlockDto * block, const BlockSidebandDto * sideband);

uintptr_t rsn_block_sideband_size (uint8_t block_type, int32_t * result);

int32_t rsn_bootstrap_constants_create (const NetworkConstantsDto * network_constants,
BootstrapConstantsDto * dto);

void rsn_callback_blake2b_final (Blake2BFinalCallback f);

void rsn_callback_blake2b_init (Blake2BInitCallback f);

void rsn_callback_blake2b_update (Blake2BUpdateCallback f);

void rsn_callback_property_tree_get_string (PropertyTreeGetStringCallback f);

void rsn_callback_property_tree_put_string (PropertyTreePutStringCallback f);

void rsn_callback_read_bytes (ReadBytesCallback f);

void rsn_callback_read_u8 (ReadU8Callback f);

void rsn_callback_toml_array_put_str (TomlArrayPutStrCallback f);

void rsn_callback_toml_create_array (TomlCreateArrayCallback f);

void rsn_callback_toml_create_config (TomlCreateConfigCallback f);

void rsn_callback_toml_drop_array (TomlDropArrayCallback f);

void rsn_callback_toml_drop_config (TomlDropConfigCallback f);

void rsn_callback_toml_put_bool (TomlPutBoolCallback f);

void rsn_callback_toml_put_child (TomlPutChildCallback f);

void rsn_callback_toml_put_f64 (TomlPutF64Callback f);

void rsn_callback_toml_put_i64 (TomlPutI64Callback f);

void rsn_callback_toml_put_str (TomlPutStrCallback f);

void rsn_callback_toml_put_u64 (TomlPutU64Callback f);

void rsn_callback_write_bytes (WriteBytesCallback f);

void rsn_callback_write_u8 (WriteU8Callback f);

ChangeBlockHandle * rsn_change_block_clone (const ChangeBlockHandle * handle);

ChangeBlockHandle * rsn_change_block_create (const ChangeBlockDto * dto);

ChangeBlockHandle * rsn_change_block_create2 (const ChangeBlockDto2 * dto);

ChangeBlockHandle * rsn_change_block_deserialize (void * stream);

ChangeBlockHandle * rsn_change_block_deserialize_json (const void * ptree);

void rsn_change_block_destroy (ChangeBlockHandle * handle);

bool rsn_change_block_equals (const ChangeBlockHandle * a, const ChangeBlockHandle * b);

void rsn_change_block_hash (const ChangeBlockHandle * handle, uint8_t (*hash)[32]);

void rsn_change_block_previous (const ChangeBlockHandle * handle, uint8_t (*result)[32]);

void rsn_change_block_previous_set (ChangeBlockHandle * handle, const uint8_t (*source)[32]);

void rsn_change_block_representative (const ChangeBlockHandle * handle, uint8_t (*result)[32]);

void rsn_change_block_representative_set (ChangeBlockHandle * handle,
const uint8_t (*representative)[32]);

int32_t rsn_change_block_serialize (ChangeBlockHandle * handle, void * stream);

int32_t rsn_change_block_serialize_json (const ChangeBlockHandle * handle, void * ptree);

void rsn_change_block_signature (const ChangeBlockHandle * handle, uint8_t (*result)[64]);

void rsn_change_block_signature_set (ChangeBlockHandle * handle, const uint8_t (*signature)[64]);

uintptr_t rsn_change_block_size ();

uint64_t rsn_change_block_work (const ChangeBlockHandle * handle);

void rsn_change_block_work_set (ChangeBlockHandle * handle, uint64_t work);

int32_t rsn_daemon_config_create (DaemonConfigDto * dto, const NetworkParamsDto * network_params);

int32_t rsn_daemon_config_serialize_toml (const DaemonConfigDto * dto, void * toml);

int32_t rsn_deserialize_block_json (BlockDto * dto, const void * ptree);

uint64_t rsn_difficulty_from_multiplier (double multiplier, uint64_t base_difficulty);

double rsn_difficulty_to_multiplier (uint64_t difficulty, uint64_t base_difficulty);

int32_t rsn_ipc_config_create (IpcConfigDto * dto, const NetworkConstantsDto * network_constants);

int32_t rsn_ledger_constants_create (LedgerConstantsDto * dto,
const WorkThresholdsDto * work,
uint16_t network);

void rsn_lmdb_config_create (LmdbConfigDto * dto);

void rsn_logging_create (LoggingDto * dto);

uint16_t rsn_network_constants_active_network ();

void rsn_network_constants_active_network_set (uint16_t network);

int32_t rsn_network_constants_active_network_set_str (const char * network);

int64_t rsn_network_constants_cleanup_cutoff_s (const NetworkConstantsDto * dto);

int64_t rsn_network_constants_cleanup_period_half_ms (const NetworkConstantsDto * dto);

int32_t rsn_network_constants_create (NetworkConstantsDto * dto,
const WorkThresholdsDto * work,
uint16_t network);

bool rsn_network_constants_is_beta_network (const NetworkConstantsDto * dto);

bool rsn_network_constants_is_dev_network (const NetworkConstantsDto * dto);

bool rsn_network_constants_is_live_network (const NetworkConstantsDto * dto);

bool rsn_network_constants_is_test_network (const NetworkConstantsDto * dto);

int32_t rsn_network_params_create (NetworkParamsDto * dto, uint16_t network);

int32_t rsn_node_config_create (NodeConfigDto * dto,
uint16_t peering_port,
bool peering_port_defined,
const LoggingDto * logging,
const NetworkParamsDto * network_params);

int32_t rsn_node_config_serialize_toml (const NodeConfigDto * dto, void * toml);

int32_t rsn_node_constants_create (const NetworkConstantsDto * network_constants,
NodeConstantsDto * dto);

int32_t rsn_node_rpc_config_create (NodeRpcConfigDto * dto);

void rsn_open_block_account (const OpenBlockHandle * handle, uint8_t (*result)[32]);

void rsn_open_block_account_set (OpenBlockHandle * handle, const uint8_t (*account)[32]);

OpenBlockHandle * rsn_open_block_clone (const OpenBlockHandle * handle);

OpenBlockHandle * rsn_open_block_create (const OpenBlockDto * dto);

OpenBlockHandle * rsn_open_block_create2 (const OpenBlockDto2 * dto);

OpenBlockHandle * rsn_open_block_deserialize (void * stream);

OpenBlockHandle * rsn_open_block_deserialize_json (const void * ptree);

void rsn_open_block_destroy (OpenBlockHandle * handle);

bool rsn_open_block_equals (const OpenBlockHandle * a, const OpenBlockHandle * b);

void rsn_open_block_hash (const OpenBlockHandle * handle, uint8_t (*hash)[32]);

void rsn_open_block_representative (const OpenBlockHandle * handle, uint8_t (*result)[32]);

void rsn_open_block_representative_set (OpenBlockHandle * handle,
const uint8_t (*representative)[32]);

int32_t rsn_open_block_serialize (OpenBlockHandle * handle, void * stream);

int32_t rsn_open_block_serialize_json (const OpenBlockHandle * handle, void * ptree);

void rsn_open_block_signature (const OpenBlockHandle * handle, uint8_t (*result)[64]);

void rsn_open_block_signature_set (OpenBlockHandle * handle, const uint8_t (*signature)[64]);

uintptr_t rsn_open_block_size ();

void rsn_open_block_source (const OpenBlockHandle * handle, uint8_t (*result)[32]);

void rsn_open_block_source_set (OpenBlockHandle * handle, const uint8_t (*source)[32]);

uint64_t rsn_open_block_work (const OpenBlockHandle * handle);

void rsn_open_block_work_set (OpenBlockHandle * handle, uint64_t work);

int32_t rsn_portmapping_constants_create (const NetworkConstantsDto * network_constants,
PortmappingConstantsDto * dto);

ReceiveBlockHandle * rsn_receive_block_clone (const ReceiveBlockHandle * handle);

ReceiveBlockHandle * rsn_receive_block_create (const ReceiveBlockDto * dto);

ReceiveBlockHandle * rsn_receive_block_create2 (const ReceiveBlockDto2 * dto);

ReceiveBlockHandle * rsn_receive_block_deserialize (void * stream);

ReceiveBlockHandle * rsn_receive_block_deserialize_json (const void * ptree);

void rsn_receive_block_destroy (ReceiveBlockHandle * handle);

bool rsn_receive_block_equals (const ReceiveBlockHandle * a, const ReceiveBlockHandle * b);

void rsn_receive_block_hash (const ReceiveBlockHandle * handle, uint8_t (*hash)[32]);

void rsn_receive_block_previous (const ReceiveBlockHandle * handle, uint8_t (*result)[32]);

void rsn_receive_block_previous_set (ReceiveBlockHandle * handle, const uint8_t (*previous)[32]);

int32_t rsn_receive_block_serialize (ReceiveBlockHandle * handle, void * stream);

int32_t rsn_receive_block_serialize_json (const ReceiveBlockHandle * handle, void * ptree);

void rsn_receive_block_signature (const ReceiveBlockHandle * handle, uint8_t (*result)[64]);

void rsn_receive_block_signature_set (ReceiveBlockHandle * handle, const uint8_t (*signature)[64]);

uintptr_t rsn_receive_block_size ();

void rsn_receive_block_source (const ReceiveBlockHandle * handle, uint8_t (*result)[32]);

void rsn_receive_block_source_set (ReceiveBlockHandle * handle, const uint8_t (*previous)[32]);

uint64_t rsn_receive_block_work (const ReceiveBlockHandle * handle);

void rsn_receive_block_work_set (ReceiveBlockHandle * handle, uint64_t work);

void rsn_remove_temporary_directories ();

void rsn_rocksdb_config_create (RocksDbConfigDto * dto);

int32_t rsn_rpc_config_create (RpcConfigDto * dto, const NetworkConstantsDto * network_constants);

int32_t rsn_rpc_config_create2 (RpcConfigDto * dto,
const NetworkConstantsDto * network_constants,
uint16_t port,
bool enable_control);

int32_t rsn_rpc_config_serialize_toml (const RpcConfigDto * dto, void * toml);

void rsn_send_block_balance (const SendBlockHandle * handle, uint8_t (*result)[16]);

void rsn_send_block_balance_set (SendBlockHandle * handle, const uint8_t (*balance)[16]);

SendBlockHandle * rsn_send_block_clone (const SendBlockHandle * handle);

SendBlockHandle * rsn_send_block_create (const SendBlockDto * dto);

SendBlockHandle * rsn_send_block_create2 (const SendBlockDto2 * dto);

SendBlockHandle * rsn_send_block_deserialize (void * stream);

SendBlockHandle * rsn_send_block_deserialize_json (const void * ptree);

void rsn_send_block_destination (const SendBlockHandle * handle, uint8_t (*result)[32]);

void rsn_send_block_destination_set (SendBlockHandle * handle, const uint8_t (*destination)[32]);

void rsn_send_block_destroy (SendBlockHandle * handle);

bool rsn_send_block_equals (const SendBlockHandle * a, const SendBlockHandle * b);

void rsn_send_block_hash (const SendBlockHandle * handle, uint8_t (*hash)[32]);

void rsn_send_block_previous (const SendBlockHandle * handle, uint8_t (*result)[32]);

void rsn_send_block_previous_set (SendBlockHandle * handle, const uint8_t (*previous)[32]);

int32_t rsn_send_block_serialize (SendBlockHandle * handle, void * stream);

int32_t rsn_send_block_serialize_json (const SendBlockHandle * handle, void * ptree);

void rsn_send_block_signature (const SendBlockHandle * handle, uint8_t (*result)[64]);

void rsn_send_block_signature_set (SendBlockHandle * handle, const uint8_t (*signature)[64]);

uintptr_t rsn_send_block_size ();

bool rsn_send_block_valid_predecessor (uint8_t block_type);

uint64_t rsn_send_block_work (const SendBlockHandle * handle);

void rsn_send_block_work_set (SendBlockHandle * handle, uint64_t work);

void rsn_send_block_zero (SendBlockHandle * handle);

int32_t rsn_sign_message (const uint8_t (*priv_key)[32],
const uint8_t (*pub_key)[32],
const uint8_t * message,
uintptr_t len,
uint8_t (*signature)[64]);

uintptr_t rsn_signature_checker_batch_size ();

SignatureCheckerHandle * rsn_signature_checker_create (uintptr_t num_threads);

void rsn_signature_checker_destroy (SignatureCheckerHandle * handle);

bool rsn_signature_checker_verify (const SignatureCheckerHandle * handle,
SignatureCheckSetDto * check_set);

bool rsn_signature_checker_verify_batch (const SignatureCheckerHandle * handle,
SignatureCheckSetDto * check_set,
uintptr_t start_index,
uintptr_t size);

void rsn_stat_config_create (StatConfigDto * dto);

void rsn_state_block_account (const StateBlockHandle * handle, uint8_t (*result)[32]);

void rsn_state_block_account_set (StateBlockHandle * handle, const uint8_t (*source)[32]);

void rsn_state_block_balance (const StateBlockHandle * handle, uint8_t (*result)[16]);

void rsn_state_block_balance_set (StateBlockHandle * handle, const uint8_t (*balance)[16]);

StateBlockHandle * rsn_state_block_clone (const StateBlockHandle * handle);

StateBlockHandle * rsn_state_block_create (const StateBlockDto * dto);

StateBlockHandle * rsn_state_block_create2 (const StateBlockDto2 * dto);

StateBlockHandle * rsn_state_block_deserialize (void * stream);

StateBlockHandle * rsn_state_block_deserialize_json (const void * ptree);

void rsn_state_block_destroy (StateBlockHandle * handle);

bool rsn_state_block_equals (const StateBlockHandle * a, const StateBlockHandle * b);

void rsn_state_block_hash (const StateBlockHandle * handle, uint8_t (*hash)[32]);

void rsn_state_block_link (const StateBlockHandle * handle, uint8_t (*result)[32]);

void rsn_state_block_link_set (StateBlockHandle * handle, const uint8_t (*link)[32]);

void rsn_state_block_previous (const StateBlockHandle * handle, uint8_t (*result)[32]);

void rsn_state_block_previous_set (StateBlockHandle * handle, const uint8_t (*source)[32]);

void rsn_state_block_representative (const StateBlockHandle * handle, uint8_t (*result)[32]);

void rsn_state_block_representative_set (StateBlockHandle * handle,
const uint8_t (*representative)[32]);

int32_t rsn_state_block_serialize (StateBlockHandle * handle, void * stream);

int32_t rsn_state_block_serialize_json (const StateBlockHandle * handle, void * ptree);

void rsn_state_block_signature (const StateBlockHandle * handle, uint8_t (*result)[64]);

void rsn_state_block_signature_set (StateBlockHandle * handle, const uint8_t (*signature)[64]);

uintptr_t rsn_state_block_size ();

uint64_t rsn_state_block_work (const StateBlockHandle * handle);

void rsn_state_block_work_set (StateBlockHandle * handle, uint64_t work);

uint16_t rsn_test_node_port ();

void rsn_txn_tracking_config_create (TxnTrackingConfigDto * dto);

int32_t rsn_unique_path (uint16_t network, uint8_t * result, uintptr_t size);

bool rsn_using_rocksdb_in_tests ();

bool rsn_validate_batch (const uint8_t * const * messages,
const uintptr_t * message_lengths,
const uint8_t * const * public_keys,
const uint8_t * const * signatures,
uintptr_t num,
int32_t * valid);

bool rsn_validate_message (const uint8_t (*pub_key)[32],
const uint8_t * message,
uintptr_t len,
const uint8_t (*signature)[64]);

int32_t rsn_voting_constants_create (const NetworkConstantsDto * network_constants,
VotingConstantsDto * dto);

int32_t rsn_websocket_config_create (WebsocketConfigDto * dto, const NetworkConstantsDto * network);

void rsn_work_thresholds_create (WorkThresholdsDto * dto,
uint64_t epoch_1,
uint64_t epoch_2,
uint64_t epoch_2_receive);

double rsn_work_thresholds_denormalized_multiplier (const WorkThresholdsDto * dto,
double multiplier,
uint64_t threshold);

uint64_t rsn_work_thresholds_difficulty (const WorkThresholdsDto * dto,
uint8_t work_version,
const uint8_t (*root)[32],
uint64_t work);

double rsn_work_thresholds_normalized_multiplier (const WorkThresholdsDto * dto,
double multiplier,
uint64_t threshold);

void rsn_work_thresholds_publish_beta (WorkThresholdsDto * dto);

void rsn_work_thresholds_publish_dev (WorkThresholdsDto * dto);

void rsn_work_thresholds_publish_full (WorkThresholdsDto * dto);

void rsn_work_thresholds_publish_test (WorkThresholdsDto * dto);

uint64_t rsn_work_thresholds_threshold (const WorkThresholdsDto * dto,
const BlockDetailsDto * details);

uint64_t rsn_work_thresholds_threshold2 (const WorkThresholdsDto * dto,
uint8_t work_version,
const BlockDetailsDto * details);

uint64_t rsn_work_thresholds_threshold_base (const WorkThresholdsDto * dto, uint8_t work_version);

uint64_t rsn_work_thresholds_threshold_entry (const WorkThresholdsDto * dto,
uint8_t work_version,
uint8_t block_type);

bool rsn_work_thresholds_validate_entry (const WorkThresholdsDto * dto,
uint8_t work_version,
const uint8_t (*root)[32],
uint64_t work);

uint64_t rsn_work_thresholds_value (const WorkThresholdsDto * dto,
const uint8_t (*root)[32],
uint64_t work);

int32_t rsn_working_path (uint16_t network, uint8_t * result, uintptr_t size);

} // extern "C"

} // namespace rsnano

#endif // rs_nano_bindings_hpp
