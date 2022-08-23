#include <nano/lib/jsonconfig.hpp>
#include <nano/lib/logger_mt.hpp>
#include <nano/lib/threading.hpp>
#include <nano/lib/utility.hpp>
#include <nano/node/lmdb/lmdb_env.hpp>
#include <nano/node/lmdb/lmdb_txn.hpp>
#include <nano/secure/store.hpp>

#include <boost/format.hpp>

// Some builds (mac) fail due to "Boost.Stacktrace requires `_Unwind_Backtrace` function".
#ifndef _WIN32
#ifdef NANO_STACKTRACE_BACKTRACE
#define BOOST_STACKTRACE_USE_BACKTRACE
#endif
#ifndef _GNU_SOURCE
#define BEFORE_GNU_SOURCE 0
#define _GNU_SOURCE
#else
#define BEFORE_GNU_SOURCE 1
#endif
#endif
// On Windows this include defines min/max macros, so keep below other includes
// to reduce conflicts with other std functions
#include <boost/stacktrace.hpp>
#ifndef _WIN32
#if !BEFORE_GNU_SOURCE
#undef _GNU_SOURCE
#endif
#endif

nano::read_mdb_txn::read_mdb_txn (uint64_t txn_id_a, MDB_env * env_a, nano::mdb_txn_callbacks txn_callbacks_a) :
	txn_handle{ rsnano::rsn_lmdb_read_txn_create (txn_id_a, reinterpret_cast<rsnano::MdbEnv *> (env_a), new nano::mdb_txn_callbacks{ txn_callbacks_a }) }
{
}

nano::read_mdb_txn::read_mdb_txn (rsnano::TransactionHandle * handle_a) :
	txn_handle{ handle_a }
{
}

nano::read_mdb_txn::~read_mdb_txn ()
{
	rsnano::rsn_lmdb_read_txn_destroy (txn_handle);
}

void nano::read_mdb_txn::reset ()
{
	rsnano::rsn_lmdb_read_txn_reset (txn_handle);
}

void nano::read_mdb_txn::renew ()
{
	rsnano::rsn_lmdb_read_txn_renew (txn_handle);
}

void nano::read_mdb_txn::refresh ()
{
	rsnano::rsn_lmdb_read_txn_refresh (txn_handle);
}

void * nano::read_mdb_txn::get_handle () const
{
	return rsnano::rsn_lmdb_read_txn_handle (txn_handle);
}

nano::write_mdb_txn::write_mdb_txn (uint64_t txn_id_a, MDB_env * env_a, nano::mdb_txn_callbacks txn_callbacks_a) :
	txn_handle{ rsnano::rsn_lmdb_write_txn_create (txn_id_a, reinterpret_cast<rsnano::MdbEnv *> (env_a), new nano::mdb_txn_callbacks{ txn_callbacks_a }) }
{
}

nano::write_mdb_txn::write_mdb_txn (rsnano::TransactionHandle * handle_a) :
	txn_handle{ handle_a }
{
}

nano::write_mdb_txn::~write_mdb_txn ()
{
	rsnano::rsn_lmdb_write_txn_destroy (txn_handle);
}

void nano::write_mdb_txn::commit ()
{
	rsnano::rsn_lmdb_write_txn_commit (txn_handle);
}

void nano::write_mdb_txn::renew ()
{
	rsnano::rsn_lmdb_write_txn_renew (txn_handle);
}

void nano::write_mdb_txn::refresh ()
{
	rsnano::rsn_lmdb_write_txn_refresh (txn_handle);
}

void * nano::write_mdb_txn::get_handle () const
{
	return rsnano::rsn_lmdb_write_txn_handle (txn_handle);
}

bool nano::write_mdb_txn::contains (nano::tables table_a) const
{
	// LMDB locks on every write
	return true;
}

nano::mdb_txn_tracker::mdb_txn_tracker (std::shared_ptr<nano::logger_mt> logger_a, nano::txn_tracking_config const & txn_tracking_config_a, std::chrono::milliseconds block_processor_batch_max_time_a)
{
	auto config_dto{ txn_tracking_config_a.to_dto () };
	handle = rsnano::rsn_mdb_txn_tracker_create (nano::to_logger_handle (logger_a), &config_dto, block_processor_batch_max_time_a.count ());
}

nano::mdb_txn_tracker::~mdb_txn_tracker ()
{
	rsnano::rsn_mdb_txn_tracker_destroy (handle);
}

void nano::mdb_txn_tracker::serialize_json (boost::property_tree::ptree & json, std::chrono::milliseconds min_read_time, std::chrono::milliseconds min_write_time)
{
	rsnano::rsn_mdb_txn_tracker_serialize_json (handle, &json, min_read_time.count (), min_write_time.count ());
}

void nano::mdb_txn_tracker::add (uint64_t txn_id, bool is_write)
{
	rsnano::rsn_mdb_txn_tracker_add (handle, txn_id, is_write);
}

/** Can be called without error if transaction does not exist */
void nano::mdb_txn_tracker::erase (uint64_t txn_id)
{
	rsnano::rsn_mdb_txn_tracker_erase (handle, txn_id);
}