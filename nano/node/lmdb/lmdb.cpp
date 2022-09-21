#include <nano/crypto_lib/random_pool.hpp>
#include <nano/lib/utility.hpp>
#include <nano/node/common.hpp>
#include <nano/node/lmdb/lmdb.hpp>
#include <nano/node/lmdb/lmdb_iterator.hpp>
#include <nano/node/lmdb/wallet_value.hpp>
#include <nano/secure/buffer.hpp>

#include <boost/filesystem.hpp>
#include <boost/format.hpp>
#include <boost/polymorphic_cast.hpp>

#include <queue>

namespace nano
{
template <>
void * mdb_val::data () const
{
	return value.mv_data;
}

template <>
std::size_t mdb_val::size () const
{
	return value.mv_size;
}

template <>
mdb_val::db_val (std::size_t size_a, void * data_a) :
	value ({ size_a, data_a })
{
}

template <>
void mdb_val::convert_buffer_to_value ()
{
	value = { buffer->size (), const_cast<uint8_t *> (buffer->data ()) };
}
}
namespace
{
rsnano::LmdbStoreHandle * create_store_handle (bool & error_a, boost::filesystem::path const & path_a, nano::mdb_env::options options_a, const std::shared_ptr<nano::logger_mt> & logger_a, nano::txn_tracking_config const & txn_tracking_config_a, std::chrono::milliseconds block_processor_batch_max_time_a)
{
	auto path_string{ path_a.string () };
	auto config_dto{ options_a.config.to_dto () };
	auto txn_config_dto{ txn_tracking_config_a.to_dto () };
	return rsnano::rsn_lmdb_store_create (&error_a, reinterpret_cast<const int8_t *> (path_string.c_str ()), &config_dto, options_a.use_no_mem_init, nano::to_logger_handle (logger_a), &txn_config_dto, block_processor_batch_max_time_a.count ());
}

void release_assert_success (int const status)
{
	nano::assert_success (status);
}
uint64_t count (nano::mdb_env const & env, nano::transaction const & transaction_a, MDB_dbi db_a)
{
	MDB_stat stats;
	auto status (mdb_stat (env.tx (transaction_a), db_a, &stats));
	release_assert_success (status);
	return (stats.ms_entries);
}
}

nano::lmdb::store::store (std::shared_ptr<nano::logger_mt> logger_a, boost::filesystem::path const & path_a, nano::ledger_constants & constants, nano::txn_tracking_config const & txn_tracking_config_a, std::chrono::milliseconds block_processor_batch_max_time_a, nano::lmdb_config const & lmdb_config_a, bool backup_before_upgrade_a) :
	handle{ create_store_handle (error, path_a, nano::mdb_env::options::make ().set_config (lmdb_config_a).set_use_no_mem_init (true), logger_a, txn_tracking_config_a, block_processor_batch_max_time_a) },
	env_m{ rsnano::rsn_lmdb_store_env (handle) },
	block_store{ rsnano::rsn_lmdb_store_block (handle) },
	frontier_store{ rsnano::rsn_lmdb_store_frontier (handle) },
	account_store{ rsnano::rsn_lmdb_store_account (handle) },
	pending_store{ rsnano::rsn_lmdb_store_pending (handle) },
	online_weight_store{ rsnano::rsn_lmdb_store_online_weight (handle) },
	pruned_store{ rsnano::rsn_lmdb_store_pruned (handle) },
	peer_store{ rsnano::rsn_lmdb_store_peer (handle) },
	confirmation_height_store{ rsnano::rsn_lmdb_store_confirmation_height (handle) },
	final_vote_store{ rsnano::rsn_lmdb_store_final_vote (handle) },
	unchecked_store{ rsnano::rsn_lmdb_store_unchecked (handle) },
	version_store{ rsnano::rsn_lmdb_store_version (handle) },
	logger (*logger_a)
{
	if (!error)
	{
		auto is_fully_upgraded (false);
		auto is_fresh_db (false);
		{
			auto transaction (tx_begin_read ());
			auto err = version_store.open_db (*transaction, 0);
			if (err == MDB_SUCCESS)
			{
				is_fully_upgraded = (version_store.get (*transaction) == version_current);
				mdb_dbi_close (env (), version_store.table_handle ());
			}
		}

		// Only open a write lock when upgrades are needed. This is because CLI commands
		// open inactive nodes which can otherwise be locked here if there is a long write
		// (can be a few minutes with the --fast_bootstrap flag for instance)
		if (!is_fully_upgraded)
		{
			if (!is_fresh_db)
			{
				logger.always_log ("Upgrade in progress...");
				if (backup_before_upgrade_a)
				{
					auto path_str{ path_a.native () };
					rsnano::rsn_lmdb_store_create_backup_file (env_m.handle, reinterpret_cast<const int8_t *> (path_str.data ()), nano::to_logger_handle (logger_a));
				}
			}
			auto needs_vacuuming = false;
			{
				auto transaction (tx_begin_write ());
				open_databases (error, *transaction, MDB_CREATE);
				if (!error)
				{
					error |= !rsnano::rsn_lmdb_store_do_upgrades (handle, transaction->get_rust_handle (), &needs_vacuuming);
				}
			}

			if (needs_vacuuming)
			{
				logger.always_log ("Preparing vacuum...");
				auto vacuum_success = vacuum_after_upgrade (path_a, lmdb_config_a);
				logger.always_log (vacuum_success ? "Vacuum succeeded." : "Failed to vacuum. (Optional) Ensure enough disk space is available for a copy of the database and try to vacuum after shutting down the node");
			}
		}
		else
		{
			auto transaction (tx_begin_read ());
			open_databases (error, *transaction, 0);
		}
	}
}

nano::lmdb::store::~store ()
{
	if (handle != nullptr)
		rsnano::rsn_lmdb_store_destroy (handle);
}

bool nano::lmdb::store::vacuum_after_upgrade (boost::filesystem::path const & path_a, nano::lmdb_config const & lmdb_config_a)
{
	// Vacuum the database. This is not a required step and may actually fail if there isn't enough storage space.
	auto vacuum_path = path_a.parent_path () / "vacuumed.ldb";

	auto vacuum_success = copy_db (vacuum_path);
	if (vacuum_success)
	{
		env_m.close_env ();

		// Replace the ledger file with the vacuumed one
		boost::filesystem::rename (vacuum_path, path_a);

		// Set up the environment again
		auto options = nano::mdb_env::options::make ()
					   .set_config (lmdb_config_a)
					   .set_use_no_mem_init (true);
		env_m.init (error, path_a, options);
		if (!error)
		{
			auto transaction (tx_begin_read ());
			open_databases (error, *transaction, 0);
		}
	}
	else
	{
		// The vacuum file can be in an inconsistent state if there wasn't enough space to create it
		boost::filesystem::remove (vacuum_path);
	}
	return vacuum_success;
}

void nano::lmdb::store::serialize_mdb_tracker (boost::property_tree::ptree & json, std::chrono::milliseconds min_read_time, std::chrono::milliseconds min_write_time)
{
	env_m.serialize_txn_tracker (json, min_read_time, min_write_time);
}

void nano::lmdb::store::serialize_memory_stats (boost::property_tree::ptree & json)
{
	MDB_stat stats;
	auto status (mdb_env_stat (env ().env (), &stats));
	release_assert (status == 0);
	json.put ("branch_pages", stats.ms_branch_pages);
	json.put ("depth", stats.ms_depth);
	json.put ("entries", stats.ms_entries);
	json.put ("leaf_pages", stats.ms_leaf_pages);
	json.put ("overflow_pages", stats.ms_overflow_pages);
	json.put ("page_size", stats.ms_psize);
}

std::unique_ptr<nano::write_transaction> nano::lmdb::store::tx_begin_write (std::vector<nano::tables> const &, std::vector<nano::tables> const &)
{
	return env_m.tx_begin_write ();
}

std::unique_ptr<nano::read_transaction> nano::lmdb::store::tx_begin_read () const
{
	return env_m.tx_begin_read ();
}

std::string nano::lmdb::store::vendor_get () const
{
	return boost::str (boost::format ("LMDB %1%.%2%.%3%") % MDB_VERSION_MAJOR % MDB_VERSION_MINOR % MDB_VERSION_PATCH);
}

void nano::lmdb::store::open_databases (bool & error_a, nano::transaction const & transaction_a, unsigned flags)
{
	error_a |= !rsnano::rsn_lmdb_store_open_databases (handle, transaction_a.get_rust_handle (), flags);
}

bool nano::lmdb::store::copy_db (boost::filesystem::path const & destination_file)
{
	return !mdb_env_copy2 (env ().env (), destination_file.string ().c_str (), MDB_CP_COMPACT);
}

void nano::lmdb::store::rebuild_db (nano::write_transaction const & transaction_a)
{
	// Tables with uint256_union key
	std::vector<MDB_dbi> tables = { account_store.get_accounts_handle (), block_store.get_blocks_handle (), pruned_store.table_handle (), confirmation_height_store.table_handle () };
	for (auto const & table : tables)
	{
		MDB_dbi temp;
		mdb_dbi_open (env ().tx (transaction_a), "temp_table", MDB_CREATE, &temp);
		// Copy all values to temporary table
		for (auto i (nano::store_iterator<nano::uint256_union, nano::mdb_val> (std::make_unique<nano::mdb_iterator<nano::uint256_union, nano::mdb_val>> (transaction_a, table))), n (nano::store_iterator<nano::uint256_union, nano::mdb_val> (nullptr)); i != n; ++i)
		{
			auto s = mdb_put (env ().tx (transaction_a), temp, nano::mdb_val (i->first), i->second, MDB_APPEND);
			release_assert_success (s);
		}
		release_assert (count (env (), transaction_a, table) == count (env (), transaction_a, temp));
		// Clear existing table
		mdb_drop (env ().tx (transaction_a), table, 0);
		// Put values from copy
		for (auto i (nano::store_iterator<nano::uint256_union, nano::mdb_val> (std::make_unique<nano::mdb_iterator<nano::uint256_union, nano::mdb_val>> (transaction_a, temp))), n (nano::store_iterator<nano::uint256_union, nano::mdb_val> (nullptr)); i != n; ++i)
		{
			auto s = mdb_put (env ().tx (transaction_a), table, nano::mdb_val (i->first), i->second, MDB_APPEND);
			release_assert_success (s);
		}
		release_assert (count (env (), transaction_a, table) == count (env (), transaction_a, temp));
		// Remove temporary table
		mdb_drop (env ().tx (transaction_a), temp, 1);
	}
	// Pending table
	{
		MDB_dbi temp;
		mdb_dbi_open (env ().tx (transaction_a), "temp_table", MDB_CREATE, &temp);
		// Copy all values to temporary table
		for (auto i (nano::store_iterator<nano::pending_key, nano::pending_info> (std::make_unique<nano::mdb_iterator<nano::pending_key, nano::pending_info>> (transaction_a, pending_store.table_handle ()))), n (nano::store_iterator<nano::pending_key, nano::pending_info> (nullptr)); i != n; ++i)
		{
			auto s = mdb_put (env ().tx (transaction_a), temp, nano::mdb_val (i->first), nano::mdb_val (i->second), MDB_APPEND);
			release_assert_success (s);
		}
		release_assert (count (env (), transaction_a, pending_store.table_handle ()) == count (env (), transaction_a, temp));
		mdb_drop (env ().tx (transaction_a), pending_store.table_handle (), 0);
		// Put values from copy
		for (auto i (nano::store_iterator<nano::pending_key, nano::pending_info> (std::make_unique<nano::mdb_iterator<nano::pending_key, nano::pending_info>> (transaction_a, temp))), n (nano::store_iterator<nano::pending_key, nano::pending_info> (nullptr)); i != n; ++i)
		{
			auto s = mdb_put (env ().tx (transaction_a), pending_store.table_handle (), nano::mdb_val (i->first), nano::mdb_val (i->second), MDB_APPEND);
			release_assert_success (s);
		}
		release_assert (count (env (), transaction_a, pending_store.table_handle ()) == count (env (), transaction_a, temp));
		mdb_drop (env ().tx (transaction_a), temp, 1);
	}
}

bool nano::lmdb::store::init_error () const
{
	return error != MDB_SUCCESS;
}

unsigned nano::lmdb::store::max_block_write_batch_num () const
{
	return std::numeric_limits<unsigned>::max ();
}

nano::block_store & nano::lmdb::store::block ()
{
	return block_store;
}

nano::frontier_store & nano::lmdb::store::frontier ()
{
	return frontier_store;
}

nano::account_store & nano::lmdb::store::account ()
{
	return account_store;
}

nano::pending_store & nano::lmdb::store::pending ()
{
	return pending_store;
}

nano::unchecked_store & nano::lmdb::store::unchecked ()
{
	return unchecked_store;
}

nano::online_weight_store & nano::lmdb::store::online_weight ()
{
	return online_weight_store;
};

nano::pruned_store & nano::lmdb::store::pruned ()
{
	return pruned_store;
}

nano::peer_store & nano::lmdb::store::peer ()
{
	return peer_store;
}

nano::confirmation_height_store & nano::lmdb::store::confirmation_height ()
{
	return confirmation_height_store;
}

nano::final_vote_store & nano::lmdb::store::final_vote ()
{
	return final_vote_store;
}

nano::version_store & nano::lmdb::store::version ()
{
	return version_store;
}
