#include "nano/lib/logger_mt.hpp"
#include "nano/lib/threading.hpp"

#include <nano/store/lmdb/lmdb_env.hpp>

#include <boost/filesystem/operations.hpp>

rsnano::LmdbEnvHandle * create_mdb_env_handle (bool & error_a, std::filesystem::path const & path_a, nano::store::lmdb::env::options options_a)
{
	auto path_string{ path_a.string () };
	auto config_dto{ options_a.config.to_dto () };
	return rsnano::rsn_mdb_env_create (&error_a, reinterpret_cast<const int8_t *> (path_string.c_str ()), &config_dto, options_a.use_no_mem_init);
}

rsnano::LmdbEnvHandle * create_mdb_env_handle (bool & error_a, std::filesystem::path const & path_a, nano::store::lmdb::env::options options_a, const std::shared_ptr<nano::logger_mt> & logger_a, nano::txn_tracking_config const & txn_tracking_config_a, std::chrono::milliseconds block_processor_batch_max_time_a)
{
	auto path_string{ path_a.string () };
	auto config_dto{ options_a.config.to_dto () };
	auto txn_config_dto{ txn_tracking_config_a.to_dto () };
	return rsnano::rsn_mdb_env_create2 (&error_a, reinterpret_cast<const int8_t *> (path_string.c_str ()), &config_dto, options_a.use_no_mem_init, nano::to_logger_handle (logger_a), &txn_config_dto, block_processor_batch_max_time_a.count ());
}

nano::store::lmdb::env::env (bool & error_a, std::filesystem::path const & path_a, nano::store::lmdb::env::options options_a) :
	handle{ create_mdb_env_handle (error_a, path_a, options_a) }
{
}

nano::store::lmdb::env::env (bool & error_a, std::filesystem::path const & path_a, std::shared_ptr<nano::logger_mt> logger_a, nano::txn_tracking_config const & txn_tracking_config_a, std::chrono::milliseconds block_processor_batch_max_time_a, nano::store::lmdb::env::options options_a) :
	handle{ create_mdb_env_handle (error_a, path_a, options_a, logger_a, txn_tracking_config_a, block_processor_batch_max_time_a) }
{
}

nano::store::lmdb::env::env (rsnano::LmdbEnvHandle * handle_a) :
	handle{ handle_a }
{
}

nano::store::lmdb::env::~env ()
{
	if (handle != nullptr)
		rsnano::rsn_mdb_env_destroy (handle);
}

void nano::store::lmdb::env::serialize_txn_tracker (boost::property_tree::ptree & json, std::chrono::milliseconds min_read_time, std::chrono::milliseconds min_write_time)
{
	rsnano::rsn_mdb_env_serialize_txn_tracker (handle, &json, min_read_time.count (), min_write_time.count ());
}

std::unique_ptr<nano::store::read_transaction> nano::store::lmdb::env::tx_begin_read () const
{
	return std::make_unique<nano::store::lmdb::read_transaction_impl> (rsnano::rsn_mdb_env_tx_begin_read (handle));
}

std::unique_ptr<nano::store::write_transaction> nano::store::lmdb::env::tx_begin_write () const
{
	return std::make_unique<nano::store::lmdb::write_transaction_impl> (rsnano::rsn_mdb_env_tx_begin_write (handle));
}
