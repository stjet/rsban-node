#pragma once

#include <nano/lib/lmdbconfig.hpp>
#include <nano/store/component.hpp>
#include <nano/store/lmdb/transaction_impl.hpp>

namespace nano
{
class logger;
}

namespace nano::store::lmdb
{
/**
 * RAII wrapper for MDB_env
 */
class env final
{
public:
	/** Environment options, most of which originates from the config file. */
	class options final
	{
		friend class env;

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

		bool use_no_mem_init{ false };
		nano::lmdb_config config;
	};

	env (bool &, std::filesystem::path const &, nano::store::lmdb::env::options options_a = nano::store::lmdb::env::options::make ());
	env (bool &, std::filesystem::path const &, nano::txn_tracking_config const & txn_tracking_config_a, std::chrono::milliseconds block_processor_batch_max_time_a, nano::store::lmdb::env::options options_a = nano::store::lmdb::env::options::make ());
	env (rsnano::LmdbEnvHandle * handle_a);
	env (env const &) = delete;
	env (env &&) = delete;
	~env ();
	std::unique_ptr<nano::store::read_transaction> tx_begin_read () const;
	std::unique_ptr<nano::store::write_transaction> tx_begin_write () const;
	void serialize_txn_tracker (boost::property_tree::ptree & json, std::chrono::milliseconds min_read_time, std::chrono::milliseconds min_write_time);
	rsnano::LmdbEnvHandle * handle;
};
} // namespace nano::store::lmdb
