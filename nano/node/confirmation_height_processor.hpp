#pragma once

#include "nano/lib/blocks.hpp"
#include "nano/lib/rsnano.hpp"

#include <nano/lib/numbers.hpp>
#include <nano/lib/rsnanoutils.hpp>
#include <nano/lib/timer.hpp>
#include <nano/node/confirmation_height_bounded.hpp>
#include <nano/node/confirmation_height_unbounded.hpp>
#include <nano/secure/common.hpp>
#include <nano/secure/store.hpp>

#include <condition_variable>
#include <thread>
#include <unordered_set>

namespace boost
{
class latch;
}
namespace nano
{
class ledger;
class logger_mt;
class write_database_queue;

class confirmation_height_processor final
{
public:
	confirmation_height_processor (nano::ledger &, nano::stats & stats_a, nano::write_database_queue &, std::chrono::milliseconds, nano::logging const &, std::shared_ptr<nano::logger_mt> &, boost::latch & initialized_latch, confirmation_height_mode = confirmation_height_mode::automatic);
	~confirmation_height_processor ();

	class mutex_lock
	{
	public:
		mutex_lock (rsnano::ConfirmationHeightProcessorLock * handle_a) :
			handle{ handle_a }
		{
		}
		mutex_lock (mutex_lock && other)
		{
			handle = other.handle;
			other.handle = nullptr;
		}
		mutex_lock (mutex_lock const &) = delete;
		~mutex_lock ()
		{
			if (handle)
			{
				rsnano::rsn_confirmation_height_processor_lock_destroy (handle);
			}
		}
		void unlock ()
		{
			rsnano::rsn_confirmation_height_processor_lock_unlock (handle);
		}
		void lock ()
		{
			rsnano::rsn_confirmation_height_processor_lock_relock (handle);
		}

		bool paused () const
		{
			return rsnano::rsn_confirmation_height_processor_lock_paused (handle);
		}

		void set_paused (bool value)
		{
			rsnano::rsn_confirmation_height_processor_lock_paused_set (handle, value);
		}

		void awaiting_processing_push_back (std::shared_ptr<nano::block> const & block)
		{
			rsnano::rsn_confirmation_height_processor_awaiting_processing_push_back (handle, block->get_handle ());
		}

		size_t awaiting_processing_size ()
		{
			return rsnano::rsn_confirmation_height_processor_awaiting_processing_size (handle);
		}

		bool awaiting_processing_empty ()
		{
			return rsnano::rsn_confirmation_height_processor_awaiting_processing_empty (handle);
		}

		bool awaiting_processing_contains (nano::block_hash const & hash_a)
		{
			return rsnano::rsn_confirmation_height_processor_awaiting_processing_contains (handle, hash_a.bytes.data ());
		}

		std::shared_ptr<nano::block> awaiting_processing_front ()
		{
			auto block_handle = rsnano::rsn_confirmation_height_processor_awaiting_processing_front (handle);
			return nano::block_handle_to_block (block_handle);
		}

		void awaiting_processing_pop_front ()
		{
			rsnano::rsn_confirmation_height_processor_awaiting_processing_pop_front (handle);
		}

		void original_hashes_pending_insert (nano::block_hash const & hash_a)
		{
			rsnano::rsn_confirmation_height_processor_original_hashes_pending_insert (handle, hash_a.bytes.data ());
		}

		bool original_hashes_pending_contains (nano::block_hash const & hash_a)
		{
			return rsnano::rsn_confirmation_height_processor_original_hashes_pending_contains (handle, hash_a.bytes.data ());
		}

		void original_hashes_pending_clear ()
		{
			rsnano::rsn_confirmation_height_processor_original_hashes_pending_clear (handle);
		}

		std::shared_ptr<nano::block> original_block () const
		{
			auto block_handle = rsnano::rsn_confirmation_height_processor_original_block (handle);
			return nano::block_handle_to_block (block_handle);
		}

		void set_original_block (std::shared_ptr<nano::block> const & block_a)
		{
			rsnano::BlockHandle * block_handle = nullptr;
			if (block_a)
			{
				block_handle = block_a->get_handle ();
			}
			rsnano::rsn_confirmation_height_processor_original_block_set (handle, block_handle);
		}

		rsnano::ConfirmationHeightProcessorLock * handle;
	};

	class mutex_wrapper
	{
	public:
		mutex_wrapper (rsnano::ConfirmationHeightProcessorMutex * handle_a) :
			handle{ handle_a }
		{
		}
		mutex_wrapper (mutex_wrapper const &) = delete;
		~mutex_wrapper ()
		{
			rsnano::rsn_confirmation_height_processor_mutex_destroy (handle);
		}

		mutex_lock lock ()
		{
			return mutex_lock{ rsnano::rsn_confirmation_height_processor_mutex_lock (handle) };
		}

		rsnano::ConfirmationHeightProcessorMutex * handle;
	};

	class condvar_wrapper
	{
	public:
		condvar_wrapper (rsnano::ConfirmationHeightProcessorCondvar * handle_a) :
			handle{ handle_a }
		{
		}
		condvar_wrapper (condvar_wrapper const &) = delete;
		~condvar_wrapper ()
		{
			rsnano::rsn_confirmation_height_processor_condvar_destroy (handle);
		}
		void wait (mutex_lock & lk)
		{
			rsnano::rsn_confirmation_height_processor_condvar_wait (handle, lk.handle);
		}

		void notify_one ()
		{
			rsnano::rsn_confirmation_height_processor_condvar_notify_one (handle);
		}

		rsnano::ConfirmationHeightProcessorCondvar * handle;
	};

	void pause ();
	void unpause ();
	void stop ();
	void add (std::shared_ptr<nano::block> const &);
	void run (confirmation_height_mode);
	std::size_t awaiting_processing_size () const;
	bool is_processing_added_block (nano::block_hash const & hash_a) const;
	bool is_processing_block (nano::block_hash const &) const;
	nano::block_hash current () const;

	/*
	 * Called for each newly cemented block
	 * Called from confirmation height processor thread
	 */
	void set_cemented_observer (std::function<void (std::shared_ptr<nano::block> const &)> const &);
	void clear_cemented_observer ();
	/*
	 * Called when the block was added to the confirmation height processor but is already confirmed
	 * Called from confirmation height processor thread
	 */
	void add_block_already_cemented_observer (std::function<void (nano::block_hash const &)> const &);

private:
	// No mutex needed for the observers as these should be set up during initialization of the node
	std::vector<std::function<void (nano::block_hash const &)>> block_already_cemented_observers;

	nano::ledger & ledger;
	nano::write_database_queue & write_database_queue;

	rsnano::ConfirmationHeightProcessorHandle * handle;
	rsnano::AtomicBoolWrapper stopped;
	mutable mutex_wrapper mutex;
	condvar_wrapper condition;
	/** The maximum amount of blocks to write at once. This is dynamically modified by the bounded processor based on previous write performance **/
	rsnano::AtomicU64Wrapper batch_write_size;

	confirmation_height_unbounded unbounded_processor;
	confirmation_height_bounded bounded_processor;
	std::thread thread;

	void set_next_hash ();
	void notify_cemented (std::vector<std::shared_ptr<nano::block>> const &);
	void notify_already_cemented (nano::block_hash const &);

	friend std::unique_ptr<container_info_component> collect_container_info (confirmation_height_processor &, std::string const &);

private: // Tests
	friend class confirmation_height_pending_observer_callbacks_Test;
	friend class confirmation_height_dependent_election_Test;
	friend class confirmation_height_dependent_election_after_already_cemented_Test;
	friend class confirmation_height_dynamic_algorithm_no_transition_while_pending_Test;
	friend class confirmation_height_many_accounts_many_confirmations_Test;
	friend class confirmation_height_long_chains_Test;
	friend class confirmation_height_many_accounts_single_confirmation_Test;
	friend class request_aggregator_cannot_vote_Test;
	friend class active_transactions_pessimistic_elections_Test;
};

std::unique_ptr<container_info_component> collect_container_info (confirmation_height_processor &, std::string const &);
}
