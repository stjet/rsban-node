#pragma once

#include <nano/crypto_lib/random_pool.hpp>
#include <nano/lib/memory.hpp>
#include <nano/secure/buffer.hpp>
#include <nano/secure/common.hpp>
#include <nano/store/tables.hpp>
#include <nano/store/transaction.hpp>

#include <boost/endian/conversion.hpp>
#include <boost/polymorphic_cast.hpp>

#include <stack>

namespace nano
{
class account_store;
class block_store;
class confirmation_height_store;
class final_vote_store;
class frontier_store;
class ledger_cache;
class online_weight_store;
class peer_store;
class pending_store;
class pruned_store;
class version_store;

namespace store
{
	/**
 * Store manager
 */
	class component
	{
	public:
		virtual ~component () = default;
		virtual block_store & block () = 0;
		virtual frontier_store & frontier () = 0;
		virtual account_store & account () = 0;
		virtual pending_store & pending () = 0;
		virtual online_weight_store & online_weight () = 0;
		virtual pruned_store & pruned () = 0;
		virtual peer_store & peer () = 0;
		virtual confirmation_height_store & confirmation_height () = 0;
		virtual final_vote_store & final_vote () = 0;
		virtual version_store & version () = 0;
		static int constexpr version_minimum{ 21 };
		static int constexpr version_current{ 22 };

		virtual unsigned max_block_write_batch_num () const = 0;

		virtual bool copy_db (boost::filesystem::path const & destination) = 0;
		virtual void rebuild_db (nano::write_transaction const & transaction_a) = 0;

		/** Not applicable to all sub-classes */
		virtual void serialize_mdb_tracker (boost::property_tree::ptree &, std::chrono::milliseconds, std::chrono::milliseconds){};
		virtual void serialize_memory_stats (boost::property_tree::ptree &) = 0;

		virtual bool init_error () const = 0;

		/** Start read-write transaction */
		virtual std::unique_ptr<nano::write_transaction> tx_begin_write (std::vector<nano::tables> const & tables_to_lock = {}, std::vector<nano::tables> const & tables_no_lock = {}) = 0;

		/** Start read-only transaction */
		virtual std::unique_ptr<nano::read_transaction> tx_begin_read () const = 0;

		virtual std::string vendor_get () const = 0;
		virtual rsnano::LmdbStoreHandle * get_handle () const = 0;
	};
} // namespace store
} // namespace nano
