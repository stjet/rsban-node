#include "nano/lib/rsnano.hpp"

#include <nano/lib/logger_mt.hpp>
#include <nano/lib/numbers.hpp>
#include <nano/lib/threading.hpp>
#include <nano/lib/utility.hpp>
#include <nano/node/confirmation_height_processor.hpp>
#include <nano/node/write_database_queue.hpp>
#include <nano/secure/common.hpp>
#include <nano/secure/ledger.hpp>

#include <boost/thread/latch.hpp>

nano::confirmation_height_processor::confirmation_height_processor (nano::ledger & ledger_a, nano::stats & stats_a, nano::write_database_queue & write_database_queue_a, std::chrono::milliseconds batch_separate_pending_min_time_a, nano::logging const & logging_a, std::shared_ptr<nano::logger_mt> & logger_a, boost::latch & latch, confirmation_height_mode mode_a) :
	ledger (ledger_a),
	write_database_queue (write_database_queue_a),
	unbounded_processor (
	ledger_a, stats_a, write_database_queue_a, batch_separate_pending_min_time_a, logging_a, logger_a, batch_write_size,
	/* cemented_callback */ [this] (auto & cemented_blocks) { this->notify_cemented (cemented_blocks); },
	/* already cemented_callback */ [this] (auto const & block_hash_a) { this->notify_already_cemented (block_hash_a); },
	/* awaiting_processing_size_query */ [this] () { return this->awaiting_processing_size (); }),
	bounded_processor (
	ledger_a, write_database_queue_a, batch_separate_pending_min_time_a, logging_a, logger_a, stopped, batch_write_size,
	/* cemented_callback */ [this] (auto & cemented_blocks) { this->notify_cemented (cemented_blocks); },
	/* already cemented_callback */ [this] (auto const & block_hash_a) { this->notify_already_cemented (block_hash_a); },
	/* awaiting_processing_size_query */ [this] () { return this->awaiting_processing_size (); }),
	thread ([this, &latch, mode_a] () {
		nano::thread_role::set (nano::thread_role::name::confirmation_height_processing);
		// Do not start running the processing thread until other threads have finished their operations
		latch.wait ();
		this->run (mode_a);
	}),
	handle{ rsnano::rsn_confirmation_height_processor_create (write_database_queue_a.handle) },
	mutex{ rsnano::rsn_confirmation_height_processor_get_mutex (handle) },
	condition{ rsnano::rsn_confirmation_height_processor_get_condvar (handle) }
{
}

nano::confirmation_height_processor::~confirmation_height_processor ()
{
	stop ();
	rsnano::rsn_confirmation_height_processor_destroy (handle);
}

void nano::confirmation_height_processor::stop ()
{
	{
		auto guard{ mutex.lock () };
		stopped.store (true);
		unbounded_processor.stop ();
	}
	condition.notify_one ();
	if (thread.joinable ())
	{
		thread.join ();
	}
}

void nano::confirmation_height_processor::run (confirmation_height_mode mode_a)
{
	auto lk{ mutex.lock () };
	while (!stopped.load ())
	{
		if (!lk.paused () && !lk.awaiting_processing_empty ())
		{
			lk.unlock ();
			if (bounded_processor.pending_empty () && unbounded_processor.pending_empty ())
			{
				lk.lock ();
				lk.original_hashes_pending_clear ();
				lk.unlock ();
			}

			set_next_hash ();

			auto const num_blocks_to_use_unbounded = confirmation_height::unbounded_cutoff;
			auto blocks_within_automatic_unbounded_selection = (ledger.cache.block_count () < num_blocks_to_use_unbounded || ledger.cache.block_count () - num_blocks_to_use_unbounded < ledger.cache.cemented_count ());

			// Don't want to mix up pending writes across different processors
			auto valid_unbounded = (mode_a == confirmation_height_mode::automatic && blocks_within_automatic_unbounded_selection && bounded_processor.pending_empty ());
			auto force_unbounded = (!unbounded_processor.pending_empty () || mode_a == confirmation_height_mode::unbounded);
			if (force_unbounded || valid_unbounded)
			{
				debug_assert (bounded_processor.pending_empty ());
				lk.lock ();
				auto original_block = lk.original_block ();
				lk.unlock ();
				unbounded_processor.process (original_block);
			}
			else
			{
				debug_assert (mode_a == confirmation_height_mode::bounded || mode_a == confirmation_height_mode::automatic);
				debug_assert (unbounded_processor.pending_empty ());
				lk.lock ();
				auto original_block = lk.original_block ();
				lk.unlock ();
				bounded_processor.process (original_block);
			}

			lk.lock ();
		}
		else
		{
			auto lock_and_cleanup = [&lk, this] () {
				lk.lock ();
				lk.set_original_block (nullptr);
				lk.original_hashes_pending_clear ();
				bounded_processor.clear_process_vars ();
				unbounded_processor.clear_process_vars ();
			};

			if (!lk.paused ())
			{
				lk.unlock ();

				// If there are blocks pending cementing, then make sure we flush out the remaining writes
				if (!bounded_processor.pending_empty ())
				{
					debug_assert (unbounded_processor.pending_empty ());
					{
						auto scoped_write_guard = write_database_queue.wait (nano::writer::confirmation_height);
						bounded_processor.cement_blocks (scoped_write_guard);
					}
					lock_and_cleanup ();
				}
				else if (!unbounded_processor.pending_empty ())
				{
					debug_assert (bounded_processor.pending_empty ());
					{
						unbounded_processor.cement_blocks ();
					}
					lock_and_cleanup ();
				}
				else
				{
					lock_and_cleanup ();
					// A block could have been confirmed during the re-locking
					if (lk.awaiting_processing_empty ())
					{
						condition.wait (lk);
					}
				}
			}
			else
			{
				// Pausing is only utilised in some tests to help prevent it processing added blocks until required.
				lk.set_original_block (nullptr);
				condition.wait (lk);
			}
		}
	}
}

// Pausing only affects processing new blocks, not the current one being processed. Currently only used in tests
void nano::confirmation_height_processor::pause ()
{
	rsnano::rsn_confirmation_height_processor_pause (handle);
}

void nano::confirmation_height_processor::unpause ()
{
	rsnano::rsn_confirmation_height_processor_unpause (handle);
}

void nano::confirmation_height_processor::add (std::shared_ptr<nano::block> const & block_a)
{
	rsnano::rsn_confirmation_height_processor_add (handle, block_a->get_handle ());
}

void nano::confirmation_height_processor::set_next_hash ()
{
	auto lk{ mutex.lock () };
	debug_assert (!lk.awaiting_processing_empty ());
	auto original_block = lk.awaiting_processing_front ();
	lk.set_original_block (original_block);
	lk.original_hashes_pending_insert (original_block->hash ());
	lk.awaiting_processing_pop_front ();
}

// Not thread-safe, only call before this processor has begun cementing
void nano::confirmation_height_processor::add_cemented_observer (std::function<void (std::shared_ptr<nano::block> const &)> const & callback_a)
{
	cemented_observers.push_back (callback_a);
}

// Not thread-safe, only call before this processor has begun cementing
void nano::confirmation_height_processor::add_block_already_cemented_observer (std::function<void (nano::block_hash const &)> const & callback_a)
{
	block_already_cemented_observers.push_back (callback_a);
}

void nano::confirmation_height_processor::notify_cemented (std::vector<std::shared_ptr<nano::block>> const & cemented_blocks)
{
	for (auto const & block_callback_data : cemented_blocks)
	{
		for (auto const & observer : cemented_observers)
		{
			observer (block_callback_data);
		}
	}
}

void nano::confirmation_height_processor::notify_already_cemented (nano::block_hash const & hash_already_cemented_a)
{
	for (auto const & observer : block_already_cemented_observers)
	{
		observer (hash_already_cemented_a);
	}
}

std::unique_ptr<nano::container_info_component> nano::collect_container_info (confirmation_height_processor & confirmation_height_processor_a, std::string const & name_a)
{
	auto composite = std::make_unique<container_info_composite> (name_a);

	std::size_t cemented_observers_count = confirmation_height_processor_a.cemented_observers.size ();
	std::size_t block_already_cemented_observers_count = confirmation_height_processor_a.block_already_cemented_observers.size ();
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "cemented_observers", cemented_observers_count, sizeof (decltype (confirmation_height_processor_a.cemented_observers)::value_type) }));
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "block_already_cemented_observers", block_already_cemented_observers_count, sizeof (decltype (confirmation_height_processor_a.block_already_cemented_observers)::value_type) }));
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "awaiting_processing", confirmation_height_processor_a.awaiting_processing_size (), rsnano::rsn_confirmation_height_processor_awaiting_processing_entry_size () }));
	composite->add_component (collect_container_info (confirmation_height_processor_a.bounded_processor, "bounded_processor"));
	composite->add_component (collect_container_info (confirmation_height_processor_a.unbounded_processor, "unbounded_processor"));
	return composite;
}

std::size_t nano::confirmation_height_processor::awaiting_processing_size () const
{
	auto lk{ mutex.lock () };
	return lk.awaiting_processing_size ();
}

bool nano::confirmation_height_processor::is_processing_added_block (nano::block_hash const & hash_a) const
{
	auto lk{ mutex.lock () };
	return lk.original_hashes_pending_contains (hash_a) || lk.awaiting_processing_contains (hash_a);
}

bool nano::confirmation_height_processor::is_processing_block (nano::block_hash const & hash_a) const
{
	return is_processing_added_block (hash_a) || unbounded_processor.has_iterated_over_block (hash_a);
}

nano::block_hash nano::confirmation_height_processor::current () const
{
	auto lk{ mutex.lock () };
	auto original_block = lk.original_block ();
	return original_block ? original_block->hash () : 0;
}
