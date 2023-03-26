#include "nano/lib/blocks.hpp"
#include "nano/lib/rsnano.hpp"
#include "nano/lib/rsnanoutils.hpp"

#include <nano/lib/logger_mt.hpp>
#include <nano/lib/numbers.hpp>
#include <nano/lib/threading.hpp>
#include <nano/lib/utility.hpp>
#include <nano/node/confirmation_height_processor.hpp>
#include <nano/node/logging.hpp>
#include <nano/node/write_database_queue.hpp>
#include <nano/secure/common.hpp>
#include <nano/secure/ledger.hpp>

#include <boost/thread/latch.hpp>

#include <cstdint>

namespace
{
rsnano::ConfirmationHeightProcessorHandle * create_processor_handle (
nano::write_database_queue & write_database_queue_a,
std::shared_ptr<nano::logger_mt> & logger_a,
nano::logging const & logging_a,
nano::ledger & ledger_a,
std::chrono::milliseconds batch_separate_pending_min_time_a,
nano::stats & stats_a)
{
	auto logging_dto{ logging_a.to_dto () };
	return rsnano::rsn_confirmation_height_processor_create (
	write_database_queue_a.handle,
	nano::to_logger_handle (logger_a),
	&logging_dto,
	ledger_a.handle,
	batch_separate_pending_min_time_a.count (),
	stats_a.handle);
}
}

nano::confirmation_height_processor::confirmation_height_processor (nano::ledger & ledger_a, nano::stats & stats_a, nano::write_database_queue & write_database_queue_a, std::chrono::milliseconds batch_separate_pending_min_time_a, nano::logging const & logging_a, std::shared_ptr<nano::logger_mt> & logger_a, boost::latch & latch, confirmation_height_mode mode_a) :
	ledger (ledger_a),
	write_database_queue (write_database_queue_a),
	handle{ create_processor_handle (write_database_queue_a, logger_a, logging_a, ledger_a, batch_separate_pending_min_time_a, stats_a) },
	mutex{ rsnano::rsn_confirmation_height_processor_get_mutex (handle) },
	condition{ rsnano::rsn_confirmation_height_processor_get_condvar (handle) },
	batch_write_size{ rsnano::rsn_confirmation_height_processor_batch_write_size (handle) },
	stopped{ rsnano::rsn_confirmation_height_processor_stopped (handle) },
	thread ([this, &latch, mode_a] () {
		nano::thread_role::set (nano::thread_role::name::confirmation_height_processing);
		// Do not start running the processing thread until other threads have finished their operations
		latch.wait ();
		this->run (mode_a);
	})
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
		rsnano::rsn_confirmation_height_processor_unbounded_stop (handle);
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
		rsnano::rsn_confirmation_height_processor_run (handle, static_cast<uint8_t> (mode_a), lk.handle);
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
	rsnano::rsn_confirmation_height_processor_set_next_hash (handle);
}

namespace
{
void block_callback (void * context_a, rsnano::BlockHandle * block_handle)
{
	auto callback = static_cast<std::function<void (std::shared_ptr<nano::block> const &)> *> (context_a);
	auto block{ nano::block_handle_to_block (rsnano::rsn_block_clone (block_handle)) };
	(*callback) (block);
}

void delete_block_callback_context (void * context_a)
{
	auto callback = static_cast<std::function<void (std::shared_ptr<nano::block> const &)> *> (context_a);
	delete callback;
}

void block_hash_callback (void * context_a, const uint8_t * hash_bytes)
{
	auto callback = static_cast<std::function<void (nano::block_hash const &)> *> (context_a);
	auto hash{ nano::block_hash::from_bytes (hash_bytes) };
	(*callback) (hash);
}

void delete_block_hash_callback_context (void * context_a)
{
	auto callback = static_cast<std::function<void (nano::block_hash const &)> *> (context_a);
	delete callback;
}
}

// Not thread-safe, only call before this processor has begun cementing
void nano::confirmation_height_processor::set_cemented_observer (std::function<void (std::shared_ptr<nano::block> const &)> const & callback_a)
{
	auto context = new std::function<void (std::shared_ptr<nano::block> const &)> (callback_a);
	rsnano::rsn_confirmation_height_processor_set_cemented_observer (handle, block_callback, context, delete_block_callback_context);
}

void nano::confirmation_height_processor::clear_cemented_observer ()
{
	rsnano::rsn_confirmation_height_processor_clear_cemented_observer (handle);
}

// Not thread-safe, only call before this processor has begun cementing
void nano::confirmation_height_processor::set_block_already_cemented_observer (std::function<void (nano::block_hash const &)> const & callback_a)
{
	auto context = new std::function<void (nano::block_hash const &)> (callback_a);
	rsnano::rsn_confirmation_height_processor_set_already_cemented_observer (handle, block_hash_callback, context, delete_block_hash_callback_context);
}

size_t nano::confirmation_height_processor::unbounded_pending_writes_size () const
{
	return rsnano::rsn_confirmation_height_processor_unbounded_pending_writes (handle);
}

void nano::confirmation_height_processor::notify_cemented (std::vector<std::shared_ptr<nano::block>> const & cemented_blocks)
{
	rsnano::block_vec wrapped_blocks{ cemented_blocks };
	rsnano::rsn_confirmation_height_processor_notify_cemented (handle, wrapped_blocks.handle);
}

void nano::confirmation_height_processor::notify_already_cemented (nano::block_hash const & hash_already_cemented_a)
{
	rsnano::rsn_confirmation_height_processor_notify_already_cemented (handle, hash_already_cemented_a.bytes.data ());
}

std::unique_ptr<nano::container_info_component> nano::collect_bounded_container_info (confirmation_height_processor & confirmation_height_processor, std::string const & name_a)
{
	auto composite = std::make_unique<container_info_composite> (name_a);
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "pending_writes", rsnano::rsn_confirmation_height_processor_bounded_pending_len (confirmation_height_processor.handle), rsnano::rsn_confirmation_height_bounded_write_details_size () }));
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "accounts_confirmed_info", rsnano::rsn_confirmation_height_processor_bounded_accounts_confirmed_info_len (confirmation_height_processor.handle), rsnano::rsn_confirmation_height_bounded_confirmed_info_entry_size () }));
	return composite;
}

std::unique_ptr<nano::container_info_component> nano::collect_unbounded_container_info (confirmation_height_processor & confirmation_height_processor, std::string const & name_a)
{
	auto composite = std::make_unique<container_info_composite> (name_a);
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "confirmed_iterated_pairs", rsnano::rsn_confirmation_height_processor_unbounded_conf_iterated_pairs_len (confirmation_height_processor.handle), rsnano::rsn_conf_iterated_pair_size () }));
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "pending_writes", rsnano::rsn_confirmation_height_processor_unbounded_pending_writes (confirmation_height_processor.handle), rsnano::rsn_conf_height_details_size () }));
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "implicit_receive_cemented_mapping", rsnano::rsn_confirmation_height_processor_unbounded_implicit_receive_cemented_size (confirmation_height_processor.handle), rsnano::rsn_implicit_receive_cemented_mapping_value_size () }));
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "block_cache", rsnano::rsn_confirmation_height_processor_unbounded_block_cache_size (confirmation_height_processor.handle), rsnano::rsn_conf_height_unbounded_block_cache_element_size () }));
	return composite;
}

std::unique_ptr<nano::container_info_component> nano::collect_container_info (confirmation_height_processor & confirmation_height_processor_a, std::string const & name_a)
{
	auto composite = std::make_unique<container_info_composite> (name_a);

	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "cemented_observers", 1, sizeof (uintptr_t) }));
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "block_already_cemented_observers", 1, sizeof (uintptr_t) }));
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "awaiting_processing", confirmation_height_processor_a.awaiting_processing_size (), rsnano::rsn_confirmation_height_processor_awaiting_processing_entry_size () }));
	composite->add_component (collect_bounded_container_info (confirmation_height_processor_a, "bounded_processor"));
	composite->add_component (collect_unbounded_container_info (confirmation_height_processor_a, "unbounded_processor"));
	return composite;
}

std::size_t nano::confirmation_height_processor::awaiting_processing_size () const
{
	return rsnano::rsn_confirmation_height_processor_awaiting_processing_size (handle);
}

bool nano::confirmation_height_processor::is_processing_added_block (nano::block_hash const & hash_a) const
{
	return rsnano::rsn_confirmation_height_processor_is_processing_added_block (handle, hash_a.bytes.data ());
}

bool nano::confirmation_height_processor::is_processing_block (nano::block_hash const & hash_a) const
{
	return is_processing_added_block (hash_a) || rsnano::rsn_confirmation_height_processor_unbounded_has_iterated_over_block (handle, hash_a.bytes.data ());
}

nano::block_hash nano::confirmation_height_processor::current () const
{
	nano::block_hash hash;
	rsnano::rsn_confirmation_height_processor_current (handle, hash.bytes.data ());
	return hash;
}
