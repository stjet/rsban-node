#include <nano/node/make_store.hpp>
#include <nano/store/lmdb/lmdb.hpp>

#include <boost/filesystem/path.hpp>
#include <memory>

std::unique_ptr<nano::store::component> nano::make_store (
		std::shared_ptr<nano::nlogger> logger, 
		std::filesystem::path const & path, 
		nano::ledger_constants & constants, 
		bool read_only, 
		bool add_db_postfix, 
		nano::txn_tracking_config const & txn_tracking_config_a, 
		std::chrono::milliseconds block_processor_batch_max_time_a, 
		nano::lmdb_config const & lmdb_config_a, 
		bool backup_before_upgrade)
{
	return std::make_unique<nano::store::lmdb::component> (logger, add_db_postfix ? path / "data.ldb" : path, constants, txn_tracking_config_a, block_processor_batch_max_time_a, lmdb_config_a, backup_before_upgrade);
}

std::unique_ptr<nano::store::component> nano::make_store (
		std::filesystem::path const & path, 
		nano::ledger_constants & constants, 
		bool read_only, 
		bool add_db_postfix, 
		nano::txn_tracking_config const & txn_tracking_config_a, 
		std::chrono::milliseconds block_processor_batch_max_time_a, 
		nano::lmdb_config const & lmdb_config_a, 
		bool backup_before_upgrade)
{
	auto logger{std::make_shared<nano::nlogger>()};
	return nano::make_store(logger, path, constants, read_only, add_db_postfix, txn_tracking_config_a, block_processor_batch_max_time_a, lmdb_config_a, backup_before_upgrade);
}
