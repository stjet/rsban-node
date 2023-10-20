#pragma once

#include <nano/lib/diagnosticsconfig.hpp>
#include <nano/lib/lmdbconfig.hpp>
#include <nano/lib/logger_mt.hpp>
#include <nano/lib/numbers.hpp>
#include <nano/secure/common.hpp>
#include <nano/store/db_val.hpp>
#include <nano/store/lmdb/account.hpp>
#include <nano/store/lmdb/block.hpp>
#include <nano/store/lmdb/confirmation_height.hpp>
#include <nano/store/lmdb/db_val.hpp>
#include <nano/store/lmdb/final_vote.hpp>
#include <nano/store/lmdb/frontier.hpp>
#include <nano/store/lmdb/iterator.hpp>
#include <nano/store/lmdb/lmdb_env.hpp>
#include <nano/store/lmdb/online_weight.hpp>
#include <nano/store/lmdb/peer.hpp>
#include <nano/store/lmdb/pending.hpp>
#include <nano/store/lmdb/pruned.hpp>
#include <nano/store/lmdb/transaction_impl.hpp>
#include <nano/store/lmdb/version.hpp>

#include <boost/optional.hpp>

namespace nano::store::lmdb
{
/**
	 * mdb implementation of the block store
	 */
class component : public nano::store::component
{
private:
	bool error{ false };

public:
	rsnano::LmdbStoreHandle * handle;

private:
	nano::store::lmdb::account account_store;
	nano::store::lmdb::block block_store;
	nano::store::lmdb::confirmation_height confirmation_height_store;
	nano::store::lmdb::final_vote final_vote_store;
	nano::store::lmdb::frontier frontier_store;
	nano::store::lmdb::online_weight online_weight_store;
	nano::store::lmdb::peer peer_store;
	nano::store::lmdb::pending pending_store;
	nano::store::lmdb::pruned pruned_store;
	nano::store::lmdb::version version_store;

public:
	component (std::shared_ptr<nano::logger_mt>, std::filesystem::path const &, nano::ledger_constants & constants, nano::txn_tracking_config const & txn_tracking_config_a = nano::txn_tracking_config{}, std::chrono::milliseconds block_processor_batch_max_time_a = std::chrono::milliseconds (5000), nano::lmdb_config const & lmdb_config_a = nano::lmdb_config{}, bool backup_before_upgrade = false);
	~component ();
	component (store const &) = delete;
	component (store &&) = delete;
	std::unique_ptr<nano::store::write_transaction> tx_begin_write (std::vector<nano::tables> const & tables_requiring_lock = {}, std::vector<nano::tables> const & tables_no_lock = {}) override;
	std::unique_ptr<nano::store::read_transaction> tx_begin_read () const override;
	std::string vendor_get () const override;
	void serialize_mdb_tracker (boost::property_tree::ptree &, std::chrono::milliseconds, std::chrono::milliseconds) override;
	void serialize_memory_stats (boost::property_tree::ptree &) override;
	unsigned max_block_write_batch_num () const override;
	nano::store::block & block () override;
	nano::store::frontier & frontier () override;
	nano::store::account & account () override;
	nano::store::pending & pending () override;
	nano::store::online_weight & online_weight () override;
	nano::store::pruned & pruned () override;
	nano::store::peer & peer () override;
	nano::store::confirmation_height & confirmation_height () override;
	nano::store::final_vote & final_vote () override;
	nano::store::version & version () override;
	bool copy_db (std::filesystem::path const & destination_file) override;
	void rebuild_db (nano::store::write_transaction const & transaction_a) override;
	bool init_error () const override;
	rsnano::LmdbStoreHandle * get_handle () const override;
};
} // namespace nano::store::lmdb
