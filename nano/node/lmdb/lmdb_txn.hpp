#pragma once

#include <nano/lib/diagnosticsconfig.hpp>
#include <nano/lib/timer.hpp>
#include <nano/secure/store.hpp>

#include <boost/property_tree/ptree_fwd.hpp>
#include <boost/stacktrace/stacktrace_fwd.hpp>

#include <mutex>

#include <lmdb/libraries/liblmdb/lmdb.h>

namespace nano
{
class transaction;
class logger_mt;
class mdb_env;

class mdb_txn_callbacks
{
public:
	// takes a txn_id and is_write
	std::function<void (uint64_t, bool)> txn_start{ [] (uint64_t, bool) {} };

	// takes a txn_id
	std::function<void (uint64_t)> txn_end{ [] (uint64_t) {} };
};

class read_mdb_txn final : public read_transaction
{
public:
	read_mdb_txn (uint64_t txn_id_a, MDB_env * env_a, mdb_txn_callbacks mdb_txn_callbacks);
	read_mdb_txn (read_mdb_txn const &) = delete;
	read_mdb_txn (read_mdb_txn &&) = delete;
	~read_mdb_txn () override;
	void reset () override;
	void renew () override;
	void refresh () override;
	void * get_handle () const override;
	rsnano::TransactionHandle * txn_handle;
};

class write_mdb_txn final : public write_transaction
{
public:
	write_mdb_txn (uint64_t tx_id_a, MDB_env * env_a, mdb_txn_callbacks mdb_txn_callbacks);
	write_mdb_txn (write_mdb_txn const &) = delete;
	write_mdb_txn (write_mdb_txn &&) = delete;
	~write_mdb_txn () override;
	void commit () override;
	void renew () override;
	void refresh () override;
	void * get_handle () const override;
	bool contains (nano::tables table_a) const override;
	MDB_txn * handle;
	mdb_txn_callbacks txn_callbacks;
	bool active{ true };
	uint64_t txn_id;

private:
	MDB_env * env;
};

class mdb_txn_stats
{
public:
	mdb_txn_stats (uint64_t txn_id_a, bool is_write_a);
	bool is_write () const;
	nano::timer<std::chrono::milliseconds> timer;
	uint64_t txn_id;
	bool is_write_m;
	std::string thread_name;

	// Smart pointer so that we don't need the full definition which causes min/max issues on Windows
	std::shared_ptr<boost::stacktrace::stacktrace> stacktrace;
};

class mdb_txn_tracker
{
public:
	mdb_txn_tracker (nano::logger_mt & logger_a, nano::txn_tracking_config const & txn_tracking_config_a, std::chrono::milliseconds block_processor_batch_max_time_a);
	void serialize_json (boost::property_tree::ptree & json, std::chrono::milliseconds min_read_time, std::chrono::milliseconds min_write_time);
	void add (uint64_t txn_id, bool is_write);
	void erase (uint64_t txn_id);

private:
	nano::mutex mutex;
	std::vector<mdb_txn_stats> stats;
	nano::logger_mt & logger;
	nano::txn_tracking_config txn_tracking_config;
	std::chrono::milliseconds block_processor_batch_max_time;

	void log_if_held_long_enough (nano::mdb_txn_stats const & mdb_txn_stats) const;
};
}
