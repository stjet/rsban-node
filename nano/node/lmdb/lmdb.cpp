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

nano::lmdb::store::store (std::shared_ptr<nano::logger_mt> logger_a, boost::filesystem::path const & path_a, nano::ledger_constants & constants, nano::txn_tracking_config const & txn_tracking_config_a, std::chrono::milliseconds block_processor_batch_max_time_a, nano::lmdb_config const & lmdb_config_a, bool backup_before_upgrade_a) :
	// clang-format off
	env_m {error, path_a, logger_a, txn_tracking_config_a, block_processor_batch_max_time_a, nano::mdb_env::options::make ().set_config (lmdb_config_a).set_use_no_mem_init (true)},
	// clang-format on
	block_store{ *this },
	frontier_store{ *this },
	account_store{ env_m },
	pending_store{ *this },
	online_weight_store{ *this },
	pruned_store{ *this },
	peer_store{ *this },
	confirmation_height_store{ *this },
	final_vote_store{ *this },
	unchecked_store{ *this },
	version_store{ *this },
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
					create_backup_file (env (), path_a, *logger_a);
				}
			}
			auto needs_vacuuming = false;
			{
				auto transaction (tx_begin_write ());
				open_databases (error, *transaction, MDB_CREATE);
				if (!error)
				{
					error |= do_upgrades (*transaction, constants, needs_vacuuming);
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
	error_a |= frontier_store.open_db (transaction_a, flags);
	error_a |= unchecked_store.open_db (transaction_a, flags);
	error_a |= online_weight_store.open_db (transaction_a, flags);
	error_a |= version_store.open_db (transaction_a, flags);
	error_a |= peer_store.open_db (transaction_a, flags);
	error_a |= pruned_store.open_db (transaction_a, flags);
	error_a |= confirmation_height_store.open_db (transaction_a, flags);
	error_a |= account_store.open_databases (transaction_a, flags);
	error_a |= pending_store.open_db (transaction_a, flags);
	error_a |= final_vote_store.open_db (transaction_a, flags);
	error_a |= block_store.open_db (transaction_a, MDB_CREATE);
}

bool nano::lmdb::store::do_upgrades (nano::write_transaction & transaction_a, nano::ledger_constants & constants, bool & needs_vacuuming)
{
	auto error (false);
	auto version_l = version ().get (transaction_a);
	switch (version_l)
	{
		case 1:
		case 2:
		case 3:
		case 4:
		case 5:
		case 6:
		case 7:
		case 8:
		case 9:
		case 10:
		case 11:
		case 12:
		case 13:
		case 14:
		case 15:
		case 16:
		case 17:
		case 18:
		case 19:
		case 20:
			logger.always_log (boost::str (boost::format ("The version of the ledger (%1%) is lower than the minimum (%2%) which is supported for upgrades. Either upgrade to a v19, v20 or v21 node first or delete the ledger.") % version_l % version_minimum));
			error = true;
			break;
		case 21:
			break;
		default:
			logger.always_log (boost::str (boost::format ("The version of the ledger (%1%) is too high for this node") % version_l));
			error = true;
			break;
	}
	return error;
}

/** Takes a filepath, appends '_backup_<timestamp>' to the end (but before any extension) and saves that file in the same directory */
void nano::lmdb::store::create_backup_file (nano::mdb_env const & env_a, boost::filesystem::path const & filepath_a, nano::logger_mt & logger_a)
{
	auto extension = filepath_a.extension ();
	auto filename_without_extension = filepath_a.filename ().replace_extension ("");
	auto orig_filepath = filepath_a;
	auto & backup_path = orig_filepath.remove_filename ();
	auto backup_filename = filename_without_extension;
	backup_filename += "_backup_";
	backup_filename += std::to_string (std::chrono::system_clock::now ().time_since_epoch ().count ());
	backup_filename += extension;
	auto backup_filepath = backup_path / backup_filename;
	auto start_message (boost::str (boost::format ("Performing %1% backup before database upgrade...") % filepath_a.filename ()));
	logger_a.always_log (start_message);
	std::cout << start_message << std::endl;
	auto error (mdb_env_copy (env_a, backup_filepath.string ().c_str ()));
	if (error)
	{
		auto error_message (boost::str (boost::format ("%1% backup failed") % filepath_a.filename ()));
		logger_a.always_log (error_message);
		std::cerr << error_message << std::endl;
		std::exit (1);
	}
	else
	{
		auto success_message (boost::str (boost::format ("Backup created: %1%") % backup_filename));
		logger_a.always_log (success_message);
		std::cout << success_message << std::endl;
	}
}

bool nano::lmdb::store::exists (nano::transaction const & transaction_a, tables table_a, nano::mdb_val const & key_a) const
{
	nano::mdb_val junk;
	auto status = get (transaction_a, table_a, key_a, junk);
	release_assert (status == MDB_SUCCESS || status == MDB_NOTFOUND);
	return (status == MDB_SUCCESS);
}

int nano::lmdb::store::get (nano::transaction const & transaction_a, tables table_a, nano::mdb_val const & key_a, nano::mdb_val & value_a) const
{
	return mdb_get (env ().tx (transaction_a), table_to_dbi (table_a), key_a, value_a);
}

int nano::lmdb::store::put (nano::write_transaction const & transaction_a, tables table_a, nano::mdb_val const & key_a, nano::mdb_val const & value_a) const
{
	return (mdb_put (env ().tx (transaction_a), table_to_dbi (table_a), key_a, value_a, 0));
}

int nano::lmdb::store::del (nano::write_transaction const & transaction_a, tables table_a, nano::mdb_val const & key_a) const
{
	return (mdb_del (env ().tx (transaction_a), table_to_dbi (table_a), key_a, nullptr));
}

int nano::lmdb::store::drop (nano::write_transaction const & transaction_a, tables table_a)
{
	return clear (transaction_a, table_to_dbi (table_a));
}

int nano::lmdb::store::clear (nano::write_transaction const & transaction_a, MDB_dbi handle_a)
{
	return mdb_drop (env ().tx (transaction_a), handle_a, 0);
}

uint64_t nano::lmdb::store::count (nano::transaction const & transaction_a, tables table_a) const
{
	return count (transaction_a, table_to_dbi (table_a));
}

uint64_t nano::lmdb::store::count (nano::transaction const & transaction_a, MDB_dbi db_a) const
{
	MDB_stat stats;
	auto status (mdb_stat (env ().tx (transaction_a), db_a, &stats));
	release_assert_success (status);
	return (stats.ms_entries);
}

MDB_dbi nano::lmdb::store::table_to_dbi (tables table_a) const
{
	switch (table_a)
	{
		case tables::frontiers:
			return frontier_store.table_handle ();
		case tables::accounts:
			return account_store.get_accounts_handle ();
		case tables::blocks:
			return block_store.get_blocks_handle ();
		case tables::pending:
			return pending_store.table_handle ();
		case tables::unchecked:
			return unchecked_store.table_handle ();
		case tables::online_weight:
			return online_weight_store.table_handle ();
		case tables::meta:
			return version_store.table_handle ();
		case tables::peers:
			return peer_store.table_handle ();
		case tables::pruned:
			return pruned_store.table_handle ();
		case tables::confirmation_height:
			return confirmation_height_store.table_handle ();
		case tables::final_votes:
			return final_vote_store.table_handle ();
		default:
			release_assert (false);
			return peer_store.table_handle ();
	}
}

bool nano::lmdb::store::not_found (int status) const
{
	return (status_code_not_found () == status);
}

bool nano::lmdb::store::success (int status) const
{
	return (MDB_SUCCESS == status);
}

int nano::lmdb::store::status_code_not_found () const
{
	return MDB_NOTFOUND;
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
		release_assert (count (transaction_a, table) == count (transaction_a, temp));
		// Clear existing table
		mdb_drop (env ().tx (transaction_a), table, 0);
		// Put values from copy
		for (auto i (nano::store_iterator<nano::uint256_union, nano::mdb_val> (std::make_unique<nano::mdb_iterator<nano::uint256_union, nano::mdb_val>> (transaction_a, temp))), n (nano::store_iterator<nano::uint256_union, nano::mdb_val> (nullptr)); i != n; ++i)
		{
			auto s = mdb_put (env ().tx (transaction_a), table, nano::mdb_val (i->first), i->second, MDB_APPEND);
			release_assert_success (s);
		}
		release_assert (count (transaction_a, table) == count (transaction_a, temp));
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
		release_assert (count (transaction_a, pending_store.table_handle ()) == count (transaction_a, temp));
		mdb_drop (env ().tx (transaction_a), pending_store.table_handle (), 0);
		// Put values from copy
		for (auto i (nano::store_iterator<nano::pending_key, nano::pending_info> (std::make_unique<nano::mdb_iterator<nano::pending_key, nano::pending_info>> (transaction_a, temp))), n (nano::store_iterator<nano::pending_key, nano::pending_info> (nullptr)); i != n; ++i)
		{
			auto s = mdb_put (env ().tx (transaction_a), pending_store.table_handle (), nano::mdb_val (i->first), nano::mdb_val (i->second), MDB_APPEND);
			release_assert_success (s);
		}
		release_assert (count (transaction_a, pending_store.table_handle ()) == count (transaction_a, temp));
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
