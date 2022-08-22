#include "nano/lib/threading.hpp"

#include <nano/node/lmdb/lmdb_env.hpp>

#include <boost/filesystem/operations.hpp>

rsnano::LmdbEnvHandle * create_mdb_env_handle (bool & error_a, boost::filesystem::path const & path_a, nano::mdb_env::options options_a)
{
	auto path_string{ path_a.string () };
	auto config_dto{ options_a.config.to_dto () };
	return rsnano::rsn_mdb_env_create (&error_a, reinterpret_cast<const int8_t *> (path_string.c_str ()), &config_dto, options_a.use_no_mem_init);
}

nano::mdb_env::mdb_env (bool & error_a, boost::filesystem::path const & path_a, nano::mdb_env::options options_a) :
	handle{ create_mdb_env_handle (error_a, path_a, options_a) }
{
}

nano::mdb_env::~mdb_env ()
{
	auto environment = env ();
	if (environment != nullptr)
	{
		// Make sure the commits are flushed. This is a no-op unless MDB_NOSYNC is used.
		mdb_env_sync (environment, true);
		mdb_env_close (environment);
	}
	if (handle != nullptr)
		rsnano::rsn_mdb_env_destroy (handle);
}

void nano::mdb_env::init (bool & error_a, boost::filesystem::path const & path_a, nano::mdb_env::options options_a)
{
	if (handle == nullptr){
		error_a = true;
		return;
	}
	
	auto config_dto{ options_a.config.to_dto () };
	rsnano::rsn_mdb_env_init (handle, &error_a, reinterpret_cast<const int8_t *> (path_a.string ().c_str ()), &config_dto, options_a.use_no_mem_init);
}

nano::mdb_env::operator MDB_env * () const
{
	return env ();
}

std::unique_ptr<nano::read_transaction> nano::mdb_env::tx_begin_read (mdb_txn_callbacks mdb_txn_callbacks) const
{
	return std::make_unique<nano::read_mdb_txn> (next_txn_id++, env (), mdb_txn_callbacks);
}

std::unique_ptr<nano::write_transaction> nano::mdb_env::tx_begin_write (mdb_txn_callbacks mdb_txn_callbacks) const
{
	/*
	 * For IO threads, we do not want them to block on creating write transactions.
	 */
	debug_assert (nano::thread_role::get () != nano::thread_role::name::io);
	return std::make_unique<nano::write_mdb_txn> (next_txn_id++, env (), mdb_txn_callbacks);
}

MDB_txn * nano::mdb_env::tx (nano::transaction const & transaction_a) const
{
	return to_mdb_txn (transaction_a);
}

MDB_env * nano::mdb_env::env () const
{
	if (handle == nullptr)
		return nullptr;

	return static_cast<MDB_env *> (rsnano::rsn_mdb_env_get_env (handle));
}

void nano::mdb_env::close_env ()
{
	if (handle != nullptr)
		rsnano::rsn_mdb_env_close_env (handle);
}

MDB_txn * nano::to_mdb_txn (nano::transaction const & transaction_a)
{
	return static_cast<MDB_txn *> (transaction_a.get_handle ());
}

void nano::assert_success (int const status)
{
	if (status != MDB_SUCCESS)
	{
		release_assert (false, mdb_strerror (status));
	}
}

uint64_t nano::mdb_count (MDB_txn * txn, MDB_dbi db_a)
{
	MDB_stat stats;
	auto status (mdb_stat (txn, db_a, &stats));
	nano::assert_success (status);
	return (stats.ms_entries);
}
