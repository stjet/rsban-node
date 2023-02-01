#pragma once

#include <nano/lib/errors.hpp>
#include <nano/lib/observer_set.hpp>
#include <nano/lib/rsnano.hpp>
#include <nano/lib/stats_enums.hpp>
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
class stats_config final
{
public:
	void load_dto (rsnano::StatConfigDto & dto);
	[[nodiscard]] rsnano::StatConfigDto to_dto () const;
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

/** Log sink interface */
class stat_log_sink
{
public:
	explicit stat_log_sink (rsnano::StatLogSinkHandle * handle_a);
	virtual ~stat_log_sink ();

public:
	/**
	 * Returns the object representation of the log result. The type depends on the sink used.
	 * @returns Object, or nullptr if no object result is available.
	 */
	void * to_object ();

	rsnano::StatLogSinkHandle * handle;
};

/**
 * Collects counts and samples for inbound and outbound traffic, blocks, errors, and so on.
 * Stats can be queried and observed on a type level (such as message and ledger) as well as a more
 * specific detail level (such as send blocks)
 */
class stats final
{
public:
	/** Constructor using the default config values */
	stats ();
	explicit stats (rsnano::StatHandle * handle_a);
	~stats ();

	/**
	 * Initialize stats with a config.
	 * @param config Configuration object; deserialized from config.json
	 */
	explicit stats (nano::stats_config config);
	stats (nano::stats const &) = delete;
	stats (nano::stats &&) = delete;

	/**
	 * Call this to override the default sample interval and capacity, for a specific stat entry.
	 * This must be called before any stat entries are added, as part of the node initialiation.
	 */
	void configure (stat::type type, stat::detail detail, stat::dir dir, size_t interval, size_t capacity);

	/** Increments the given counter */
	void inc (stat::type type, stat::dir dir = stat::dir::in);

	/** Increments the counter for \detail, but doesn't update at the type level */
	void inc_detail_only (stat::type type, stat::detail detail, stat::dir dir = stat::dir::in);

	/** Increments the given counter */
	void inc (stat::type type, stat::detail detail, stat::dir dir = stat::dir::in);

	/** Adds \p value to the given counter */
	void add (stat::type type, stat::dir dir, uint64_t value);

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
	[[nodiscard]] std::unique_ptr<stat_log_sink> log_sink_json () const;

	/** Returns string representation of type */
	static std::string type_to_string (stat::type type);

	/** Returns string representation of detail */
	static std::string detail_to_string (stat::detail detail);

	/** Returns string representation of dir */
	static std::string dir_to_string (stat::dir detail);

	/** Stop stats being output */
	void stop ();

	rsnano::StatHandle * handle;
};
}
