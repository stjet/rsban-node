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
	read_mdb_txn (rsnano::TransactionHandle * handle_a);
	~read_mdb_txn () override;
	void reset () override;
	void renew () override;
	void refresh () override;
	void * get_handle () const override;
	rsnano::TransactionHandle * get_rust_handle () const override
	{
		return txn_handle;
	};

	rsnano::TransactionHandle * txn_handle;
};

class write_mdb_txn final : public write_transaction
{
public:
	write_mdb_txn (uint64_t tx_id_a, MDB_env * env_a, mdb_txn_callbacks mdb_txn_callbacks);
	write_mdb_txn (write_mdb_txn const &) = delete;
	write_mdb_txn (write_mdb_txn &&) = delete;
	write_mdb_txn (rsnano::TransactionHandle * handle_a);
	~write_mdb_txn () override;
	void commit () override;
	void renew () override;
	void refresh () override;
	void * get_handle () const override;
	bool contains (nano::tables table_a) const override;
	rsnano::TransactionHandle * get_rust_handle () const override
	{
		return txn_handle;
	};

	rsnano::TransactionHandle * txn_handle;
};

class mdb_txn_tracker
{
public:
	mdb_txn_tracker (std::shared_ptr<nano::logger_mt> logger_a, nano::txn_tracking_config const & txn_tracking_config_a, std::chrono::milliseconds block_processor_batch_max_time_a);
	mdb_txn_tracker (mdb_txn_tracker const &) = delete;
	mdb_txn_tracker (mdb_txn_tracker &&) = delete;
	~mdb_txn_tracker ();
	void serialize_json (boost::property_tree::ptree & json, std::chrono::milliseconds min_read_time, std::chrono::milliseconds min_write_time);
	void add (uint64_t txn_id, bool is_write);
	void erase (uint64_t txn_id);

private:
	rsnano::MdbTxnTrackerHandle * handle;
};
}
