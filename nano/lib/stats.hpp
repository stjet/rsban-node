#pragma once

#include <nano/lib/errors.hpp>
#include <nano/lib/rsnano.hpp>
#include <nano/lib/utility.hpp>

#include <boost/circular_buffer.hpp>

#include <chrono>
#include <initializer_list>
#include <map>
#include <memory>
#include <mutex>
#include <string>

namespace nano
{
class node;
class tomlconfig;
class jsonconfig;
/**
 * Serialize and deserialize the 'statistics' node from config.json
 * All configuration values have defaults. In particular, file logging of statistics
 * is disabled by default.
 */
class stat_config final
{
public:
	void load_dto (rsnano::StatConfigDto & dto);
	rsnano::StatConfigDto to_dto () const;
	/** Reads the JSON statistics node */
	nano::error deserialize_toml (nano::tomlconfig & toml);

	/** If true, sampling of counters is enabled */
	bool sampling_enabled{ false };

	/** How many sample intervals to keep in the ring buffer */
	size_t capacity{ 0 };

	/** Sample interval in milliseconds */
	size_t interval{ 0 };

	/** How often to log sample array, in milliseconds. Default is 0 (no logging) */
	size_t log_interval_samples{ 0 };

	/** How often to log counters, in milliseconds. Default is 0 (no logging) */
	size_t log_interval_counters{ 0 };

	/** Maximum number of log outputs before rotating the file */
	size_t log_rotation_count{ 100 };

	/** If true, write headers on each counter or samples writeout. The header contains log type and the current wall time. */
	bool log_headers{ true };

	/** Filename for the counter log  */
	std::string log_counters_filename{ "counters.stat" };

	/** Filename for the sampling log */
	std::string log_samples_filename{ "samples.stat" };
};

/** Value and wall time of measurement */
class stat_datapoint final
{
public:
	stat_datapoint ();
	stat_datapoint (stat_datapoint const & other_a);
	stat_datapoint (rsnano::StatDatapointHandle * handle);
	~stat_datapoint ();
	stat_datapoint & operator= (stat_datapoint const & other_a);

	uint64_t get_value () const;
	void set_value (uint64_t value_a);
	std::chrono::system_clock::time_point get_timestamp () const;
	void set_timestamp (std::chrono::system_clock::time_point timestamp_a);
	void add (uint64_t addend, bool update_timestamp = true);
	rsnano::StatDatapointHandle * handle;
};

/** Histogram values */
class stat_histogram final
{
public:
	/**
	 * Create histogram given a set of intervals and an optional bin count
	 * @param intervals_a Inclusive-exclusive intervals, e.g. {1,5,8,15} produces bins [1,4] [5,7] [8, 14]
	 * @param bin_count_a If zero (default), \p intervals_a defines all the bins. If non-zero, \p intervals_a contains the total range, which is uniformly distributed into \p bin_count_a bins.
	 */
	stat_histogram (std::initializer_list<uint64_t> intervals_a, size_t bin_count_a = 0);
	stat_histogram (rsnano::StatHistogramHandle * handle);
	stat_histogram (nano::stat_histogram const &);
	stat_histogram (nano::stat_histogram &&);
	~stat_histogram ();

	/** Add \p addend_a to the histogram bin into which \p index_a falls */
	void add (uint64_t index_a, uint64_t addend_a);

	/** Histogram bin with interval, current value and timestamp of last update */
	class bin final
	{
	public:
		bin (uint64_t start_inclusive_a, uint64_t end_exclusive_a) :
			start_inclusive (start_inclusive_a), end_exclusive (end_exclusive_a)
		{
		}
		uint64_t start_inclusive;
		uint64_t end_exclusive;
		uint64_t value{ 0 };
		std::chrono::system_clock::time_point timestamp{ std::chrono::system_clock::now () };
	};
	std::vector<bin> get_bins () const;

	rsnano::StatHistogramHandle * handle;
};

/**
 * Bookkeeping of statistics for a specific type/detail/direction combination
 */
class stat_entry final
{
public:
	stat_entry (size_t capacity, size_t interval);
	stat_entry (nano::stat_entry const &) = delete;
	stat_entry (nano::stat_entry &&) = delete;
	~stat_entry ();

	size_t get_sample_interval ();
	void set_sample_interval (size_t interval);
	void sample_current_add (uint64_t value, bool update_timestamp);
	void sample_current_set_value (uint64_t value);
	void sample_current_set_timestamp (std::chrono::system_clock::time_point value);
	nano::stat_datapoint sample_current ();
	std::vector<nano::stat_datapoint> get_samples ();
	void add_sample (nano::stat_datapoint const & sample);
	uint64_t get_counter_value ();
	std::chrono::system_clock::time_point get_counter_timestamp ();
	void counter_add (uint64_t addend, bool update_timestamp = true);
	void define_histogram (std::initializer_list<uint64_t> intervals_a, size_t bin_count_a);
	void update_histogram (uint64_t index_a, uint64_t addend_a);
	nano::stat_histogram get_histogram () const;

private:
	rsnano::StatEntryHandle * handle;
};

/** Log sink interface */
class stat_log_sink
{
public:
	stat_log_sink (rsnano::StatLogSinkHandle * handle_a);
	virtual ~stat_log_sink ();

	/** Called before logging starts */
	void begin ();

	/** Called after logging is completed */
	void finalize ();

	/** Write a header enrty to the log */
	void write_header (std::string const & header, std::chrono::system_clock::time_point & walltime);

	/** Write a counter or sampling entry to the log. Some log sinks may support writing histograms as well. */
	void write_entry (std::chrono::system_clock::time_point & time, std::string const & type, std::string const & detail, std::string const & dir, uint64_t value, nano::stat_histogram * histogram);

	/** Rotates the log (e.g. empty file). This is a no-op for sinks where rotation is not supported. */
	void rotate ();

	/** Returns a reference to the log entry counter */
	size_t entries ();

	void inc_entries ();

	/** Returns the string representation of the log. If not supported, an empty string is returned. */
	std::string to_string ();

	/**
	 * Returns the object representation of the log result. The type depends on the sink used.
	 * @returns Object, or nullptr if no object result is available.
	 */
	void * to_object ();

	rsnano::StatLogSinkHandle * handle;
};

std::string tm_to_string (tm & tm);

/**
 * Collects counts and samples for inbound and outbound traffic, blocks, errors, and so on.
 * Stats can be queried and observed on a type level (such as message and ledger) as well as a more
 * specific detail level (such as send blocks)
 */
class stat final
{
public:
	/** Primary statistics type */
	enum class type : uint8_t
	{
		traffic_udp,
		traffic_tcp,
		error,
		message,
		block,
		ledger,
		rollback,
		bootstrap,
		vote,
		election,
		http_callback,
		peering,
		ipc,
		tcp,
		udp,
		confirmation_height,
		confirmation_observer,
		drop,
		aggregator,
		requests,
		filter,
		telemetry,
		vote_generator
	};

	/** Optional detail type */
	enum class detail : uint8_t
	{
		all = 0,

		// error specific
		bad_sender,
		insufficient_work,
		http_callback,
		unreachable_host,
		invalid_network,

		// confirmation_observer specific
		active_quorum,
		active_conf_height,
		inactive_conf_height,

		// ledger, block, bootstrap
		send,
		receive,
		open,
		change,
		state_block,
		epoch_block,
		fork,
		old,
		gap_previous,
		gap_source,
		rollback_failed,

		// message specific
		keepalive,
		publish,
		republish_vote,
		confirm_req,
		confirm_ack,
		node_id_handshake,
		telemetry_req,
		telemetry_ack,

		// bootstrap, callback
		initiate,
		initiate_legacy_age,
		initiate_lazy,
		initiate_wallet_lazy,

		// bootstrap specific
		bulk_pull,
		bulk_pull_account,
		bulk_pull_deserialize_receive_block,
		bulk_pull_error_starting_request,
		bulk_pull_failed_account,
		bulk_pull_receive_block_failure,
		bulk_pull_request_failure,
		bulk_push,
		frontier_req,
		frontier_confirmation_failed,
		frontier_confirmation_successful,
		error_socket_close,
		request_underflow,

		// vote specific
		vote_valid,
		vote_replay,
		vote_indeterminate,
		vote_invalid,
		vote_overflow,

		// election specific
		vote_new,
		vote_cached,
		late_block,
		late_block_seconds,
		election_start,
		election_block_conflict,
		election_difficulty_update,
		election_drop_expired,
		election_drop_overflow,
		election_drop_all,
		election_restart,
		election_confirmed,
		election_not_confirmed,

		// udp
		blocking,
		overflow,
		invalid_header,
		invalid_message_type,
		invalid_keepalive_message,
		invalid_publish_message,
		invalid_confirm_req_message,
		invalid_confirm_ack_message,
		invalid_node_id_handshake_message,
		invalid_telemetry_req_message,
		invalid_telemetry_ack_message,
		outdated_version,
		udp_max_per_ip,
		udp_max_per_subnetwork,

		// tcp
		tcp_accept_success,
		tcp_accept_failure,
		tcp_write_drop,
		tcp_write_no_socket_drop,
		tcp_excluded,
		tcp_max_per_ip,
		tcp_max_per_subnetwork,
		tcp_silent_connection_drop,
		tcp_io_timeout_drop,
		tcp_connect_error,
		tcp_read_error,
		tcp_write_error,

		// ipc
		invocations,

		// peering
		handshake,

		// confirmation height
		blocks_confirmed,
		blocks_confirmed_unbounded,
		blocks_confirmed_bounded,

		// [request] aggregator
		aggregator_accepted,
		aggregator_dropped,

		// requests
		requests_cached_hashes,
		requests_generated_hashes,
		requests_cached_votes,
		requests_generated_votes,
		requests_cached_late_hashes,
		requests_cached_late_votes,
		requests_cannot_vote,
		requests_unknown,

		// duplicate
		duplicate_publish,

		// telemetry
		invalid_signature,
		different_genesis_hash,
		node_id_mismatch,
		request_within_protection_cache_zone,
		no_response_received,
		unsolicited_telemetry_ack,
		failed_send_telemetry_req,

		// vote generator
		generator_broadcasts,
		generator_replies,
		generator_replies_discarded,
		generator_spacing
	};

	/** Direction of the stat. If the direction is irrelevant, use in */
	enum class dir : uint8_t
	{
		in,
		out
	};

	/** Constructor using the default config values */
	stat ();
	~stat ();

	/**
	 * Initialize stats with a config.
	 * @param config Configuration object; deserialized from config.json
	 */
	stat (nano::stat_config config);
	stat (nano::stat const &) = delete;
	stat (nano::stat &&) = delete;

	/**
	 * Call this to override the default sample interval and capacity, for a specific stat entry.
	 * This must be called before any stat entries are added, as part of the node initialiation.
	 */
	void configure (stat::type type, stat::detail detail, stat::dir dir, size_t interval, size_t capacity);

	/**
	 * Disables sampling for a given type/detail/dir combination
	 */
	void disable_sampling (stat::type type, stat::detail detail, stat::dir dir);

	/** Increments the given counter */
	void inc (stat::type type, stat::dir dir = stat::dir::in);

	/** Increments the counter for \detail, but doesn't update at the type level */
	void inc_detail_only (stat::type type, stat::detail detail, stat::dir dir = stat::dir::in);

	/** Increments the given counter */
	void inc (stat::type type, stat::detail detail, stat::dir dir = stat::dir::in);

	/** Adds \p value to the given counter */
	void add (stat::type type, stat::dir dir, uint64_t value);

	/**
	 * Define histogram bins. Values are clamped into the first and last bins, but a catch-all bin on one or both
	 * ends can be defined.
	 *
	 * Examples:
	 *
	 *  // Uniform histogram, total range 12, and 12 bins (each bin has width 1)
	 *  define_histogram (type::vote, detail::confirm_ack, dir::in, {1,13}, 12);
	 *
	 *  // Specific bins matching closed intervals [1,4] [5,19] [20,99]
	 *  define_histogram (type::vote, detail::something, dir::out, {1,5,20,100});
	 *
	 *  // Logarithmic bins matching half-open intervals [1..10) [10..100) [100 1000)
	 *  define_histogram(type::vote, detail::log, dir::out, {1,10,100,1000});
	 */
	void define_histogram (stat::type type, stat::detail detail, stat::dir dir, std::initializer_list<uint64_t> intervals_a, size_t bin_count_a = 0);

	/**
	 * Update histogram
	 *
	 * Examples:
	 *
	 *  // Add 1 to the bin representing a 4-item vbh
	 *  stats.update_histogram(type::vote, detail::confirm_ack, dir::in, 4, 1)
	 *
	 *  // Add 5 to the second bin where 17 falls
	 *  stats.update_histogram(type::vote, detail::something, dir::in, 17, 5)
	 *
	 *  // Add 3 to the last bin as the histogram clamps. You can also add a final bin with maximum end value to effectively prevent this.
	 *  stats.update_histogram(type::vote, detail::log, dir::out, 1001, 3)
	 */
	void update_histogram (stat::type type, stat::detail detail, stat::dir dir, uint64_t index, uint64_t addend = 1);

	/** Returns a non-owning histogram pointer, or nullptr if a histogram is not defined */
	nano::stat_histogram get_histogram (stat::type type, stat::detail detail, stat::dir dir);

	/**
	 * Add \p value to stat. If sampling is configured, this will update the current sample and
	 * call any sample observers if the interval is over.
	 *
	 * @param type Main statistics type
	 * @param detail Detail type, or detail::none to register on type-level only
	 * @param dir Direction
	 * @param value The amount to add
	 * @param detail_only If true, only update the detail-level counter
	 */
	void add (stat::type type, stat::detail detail, stat::dir dir, uint64_t value, bool detail_only = false);

	/** Returns current value for the given counter at the type level */
	uint64_t count (stat::type type, stat::dir dir = stat::dir::in);

	/** Returns current value for the given counter at the detail level */
	uint64_t count (stat::type type, stat::detail detail, stat::dir dir = stat::dir::in);

	/** Returns the number of seconds since clear() was last called, or node startup if it's never called. */
	std::chrono::seconds last_reset ();

	/** Clear all stats */
	void clear ();

	/** Log counters to the given log link */
	void log_counters (stat_log_sink & sink);

	/** Log samples to the given log sink */
	void log_samples (stat_log_sink & sink);

	/** Returns a new JSON log sink */
	std::unique_ptr<stat_log_sink> log_sink_json () const;

	/** Returns string representation of detail */
	static std::string detail_to_string (stat::detail detail);

	/** Stop stats being output */
	void stop ();

private:
	rsnano::StatHandle * handle;
};
}
