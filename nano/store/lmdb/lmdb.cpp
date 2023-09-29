#include <nano/lib/rsnanoutils.hpp>
#include <nano/lib/stream.hpp>
#include <nano/lib/utility.hpp>
#include <nano/secure/ledger.hpp>
#include <nano/store/lmdb/iterator.hpp>
#include <nano/store/lmdb/lmdb.hpp>
#include <nano/store/lmdb/wallet_value.hpp>
#include <nano/store/version.hpp>

#include <boost/filesystem.hpp>
#include <boost/format.hpp>
#include <boost/polymorphic_cast.hpp>

#include <queue>

namespace
{
rsnano::LmdbStoreHandle * create_store_handle (bool & error_a, boost::filesystem::path const & path_a, nano::store::lmdb::env::options options_a, const std::shared_ptr<nano::logger_mt> & logger_a, nano::txn_tracking_config const & txn_tracking_config_a, std::chrono::milliseconds block_processor_batch_max_time_a, bool backup_before_upgrade)
{
	auto path_string{ path_a.string () };
	auto config_dto{ options_a.config.to_dto () };
	auto txn_config_dto{ txn_tracking_config_a.to_dto () };
	return rsnano::rsn_lmdb_store_create (&error_a, reinterpret_cast<const int8_t *> (path_string.c_str ()), &config_dto, options_a.use_no_mem_init, nano::to_logger_handle (logger_a), &txn_config_dto, block_processor_batch_max_time_a.count (), backup_before_upgrade);
}
}

nano::store::lmdb::component::component (std::shared_ptr<nano::logger_mt> logger_a, boost::filesystem::path const & path_a, nano::ledger_constants & constants, nano::txn_tracking_config const & txn_tracking_config_a, std::chrono::milliseconds block_processor_batch_max_time_a, nano::lmdb_config const & lmdb_config_a, bool backup_before_upgrade_a) :
	handle{ create_store_handle (error, path_a, nano::store::lmdb::env::options::make ().set_config (lmdb_config_a).set_use_no_mem_init (true), logger_a, txn_tracking_config_a, block_processor_batch_max_time_a, backup_before_upgrade_a) },
	block_store{ rsnano::rsn_lmdb_store_block (handle) },
	frontier_store{ rsnano::rsn_lmdb_store_frontier (handle) },
	account_store{ rsnano::rsn_lmdb_store_account (handle) },
	pending_store{ rsnano::rsn_lmdb_store_pending (handle) },
	online_weight_store{ rsnano::rsn_lmdb_store_online_weight (handle) },
	pruned_store{ rsnano::rsn_lmdb_store_pruned (handle) },
	peer_store{ rsnano::rsn_lmdb_store_peer (handle) },
	confirmation_height_store{ rsnano::rsn_lmdb_store_confirmation_height (handle) },
	final_vote_store{ rsnano::rsn_lmdb_store_final_vote (handle) },
	version_store{ rsnano::rsn_lmdb_store_version (handle) }
{
}

nano::store::lmdb::component::~component ()
{
	if (handle != nullptr)
		rsnano::rsn_lmdb_store_destroy (handle);
}

void nano::store::lmdb::component::serialize_mdb_tracker (boost::property_tree::ptree & json, std::chrono::milliseconds min_read_time, std::chrono::milliseconds min_write_time)
{
	rsnano::rsn_lmdb_store_serialize_mdb_tracker (handle, &json, min_read_time.count (), min_write_time.count ());
}

void nano::store::lmdb::component::serialize_memory_stats (boost::property_tree::ptree & json)
{
	rsnano::rsn_lmdb_store_serialize_memory_stats (handle, &json);
}

std::unique_ptr<nano::store::write_transaction> nano::store::lmdb::component::tx_begin_write (std::vector<nano::tables> const &, std::vector<nano::tables> const &)
{
	return std::make_unique<nano::store::lmdb::write_transaction_impl> (rsnano::rsn_lmdb_store_tx_begin_write (handle));
}

std::unique_ptr<nano::store::read_transaction> nano::store::lmdb::component::tx_begin_read () const
{
	return std::make_unique<nano::store::lmdb::read_transaction_impl> (rsnano::rsn_lmdb_store_tx_begin_read (handle));
}

std::string nano::store::lmdb::component::vendor_get () const
{
	rsnano::StringDto dto;
	rsnano::rsn_lmdb_store_vendor_get (handle, &dto);
	return rsnano::convert_dto_to_string (dto);
}

bool nano::store::lmdb::component::copy_db (boost::filesystem::path const & destination_file)
{
	return !rsnano::rsn_lmdb_store_copy_db (handle, reinterpret_cast<const int8_t *> (destination_file.string ().c_str ()));
}

void nano::store::lmdb::component::rebuild_db (nano::store::write_transaction const & transaction_a)
{
	rsnano::rsn_lmdb_store_rebuild_db (handle, transaction_a.get_rust_handle ());
}

bool nano::store::lmdb::component::init_error () const
{
	return error;
}

unsigned nano::store::lmdb::component::max_block_write_batch_num () const
{
	return std::numeric_limits<unsigned>::max ();
}

nano::store::block & nano::store::lmdb::component::block ()
{
	return block_store;
}

nano::store::frontier & nano::store::lmdb::component::frontier ()
{
	return frontier_store;
}

nano::store::account & nano::store::lmdb::component::account ()
{
	return account_store;
}

nano::store::pending & nano::store::lmdb::component::pending ()
{
	return pending_store;
}

nano::store::online_weight & nano::store::lmdb::component::online_weight ()
{
	return online_weight_store;
};

nano::store::pruned & nano::store::lmdb::component::pruned ()
{
	return pruned_store;
}

nano::store::peer & nano::store::lmdb::component::peer ()
{
	return peer_store;
}

nano::store::confirmation_height & nano::store::lmdb::component::confirmation_height ()
{
	return confirmation_height_store;
}

nano::store::final_vote & nano::store::lmdb::component::final_vote ()
{
	return final_vote_store;
}

nano::store::version & nano::store::lmdb::component::version ()
{
	return version_store;
}

rsnano::LmdbStoreHandle * nano::store::lmdb::component::get_handle () const
{
	return handle;
}
