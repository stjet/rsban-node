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
	std::function<void (nano::transaction const *)> txn_start{ [] (nano::transaction const *) {} };
	std::function<void (nano::transaction const *)> txn_end{ [] (nano::transaction const *) {} };
};

class read_mdb_txn final : public read_transaction
{
public:
	read_mdb_txn (MDB_env * env_a, mdb_txn_callbacks mdb_txn_callbacks);
	read_mdb_txn (read_mdb_txn const &) = delete;
	read_mdb_txn (read_mdb_txn &&) = delete;
	~read_mdb_txn ();
	void reset () override;
	void renew () override;
	void refresh () override;
	void * get_handle () const override;
	MDB_txn * handle;
	mdb_txn_callbacks txn_callbacks;
};

class write_mdb_txn final : public write_transaction
{
public:
	write_mdb_txn (MDB_env * env_a, mdb_txn_callbacks mdb_txn_callbacks);
	write_mdb_txn (write_mdb_txn const &) = delete;
	write_mdb_txn (write_mdb_txn &&) = delete;
	~write_mdb_txn ();
	void commit () override;
	void renew () override;
	void refresh () override;
	void * get_handle () const override;
	bool contains (nano::tables table_a) const override;
	MDB_txn * handle;
	mdb_txn_callbacks txn_callbacks;
	bool active{ true };
private:
	MDB_env * env;
};

class mdb_txn_stats
{
public:
	mdb_txn_stats (nano::transaction const * transaction_a);
	bool is_write () const;
	nano::timer<std::chrono::milliseconds> timer;
	nano::transaction const * transaction;
	std::string thread_name;

	// Smart pointer so that we don't need the full definition which causes min/max issues on Windows
	std::shared_ptr<boost::stacktrace::stacktrace> stacktrace;
};

class mdb_txn_tracker
{
public:
	mdb_txn_tracker (nano::logger_mt & logger_a, nano::txn_tracking_config const & txn_tracking_config_a, std::chrono::milliseconds block_processor_batch_max_time_a);
	void serialize_json (boost::property_tree::ptree & json, std::chrono::milliseconds min_read_time, std::chrono::milliseconds min_write_time);
	void add (nano::transaction const * transaction);
	void erase (nano::transaction const * transaction);

private:
	nano::mutex mutex;
	std::vector<mdb_txn_stats> stats;
	nano::logger_mt & logger;
	nano::txn_tracking_config txn_tracking_config;
	std::chrono::milliseconds block_processor_batch_max_time;

	void log_if_held_long_enough (nano::mdb_txn_stats const & mdb_txn_stats) const;
};
}
