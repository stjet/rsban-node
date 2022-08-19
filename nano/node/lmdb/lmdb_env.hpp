#pragma once

#include <nano/lib/lmdbconfig.hpp>
#include <nano/node/lmdb/lmdb_txn.hpp>
#include <nano/secure/store.hpp>

namespace nano
{
/**
 * RAII wrapper for MDB_env
 */
class mdb_env final
{
public:
	/** Environment options, most of which originates from the config file. */
	class options final
	{
		friend class mdb_env;

	public:
		static options make ()
		{
			return options ();
		}

		options & set_config (nano::lmdb_config config_a)
		{
			config = config_a;
			return *this;
		}

		options & set_use_no_mem_init (int use_no_mem_init_a)
		{
			use_no_mem_init = use_no_mem_init_a;
			return *this;
		}

		/** Used by the wallet to override the config map size */
		options & override_config_map_size (std::size_t map_size_a)
		{
			config.map_size = map_size_a;
			return *this;
		}

		/** Used by the wallet to override the sync strategy */
		options & override_config_sync (nano::lmdb_config::sync_strategy sync_a)
		{
			config.sync = sync_a;
			return *this;
		}

	private:
		bool use_no_mem_init{ false };
		nano::lmdb_config config;
	};

	mdb_env (bool &, boost::filesystem::path const &, nano::mdb_env::options options_a = nano::mdb_env::options::make ());
	mdb_env (mdb_env const &) = delete;
	mdb_env (mdb_env &&) = delete;
	void init (bool &, boost::filesystem::path const &, nano::mdb_env::options options_a = nano::mdb_env::options::make ());
	~mdb_env ();
	operator MDB_env * () const;
	std::unique_ptr<nano::read_transaction> tx_begin_read (mdb_txn_callbacks txn_callbacks = mdb_txn_callbacks{}) const;
	std::unique_ptr<nano::write_transaction> tx_begin_write (mdb_txn_callbacks txn_callbacks = mdb_txn_callbacks{}) const;
	MDB_txn * tx (nano::transaction const & transaction_a) const;
	MDB_env * env() const;
	void close_env();

private:
	MDB_env * environment;
	mutable std::atomic<uint64_t> next_txn_id{ 0 };
	rsnano::LmdbEnvHandle * handle;
};

MDB_txn * to_mdb_txn (nano::transaction const & transaction_a);
void assert_success (int const status);
uint64_t mdb_count (MDB_txn * txn, MDB_dbi db_a);
}
