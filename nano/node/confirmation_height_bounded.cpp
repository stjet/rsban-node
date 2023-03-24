#include "boost/none.hpp"
#include "nano/lib/blocks.hpp"
#include "nano/lib/numbers.hpp"
#include "nano/lib/rsnano.hpp"
#include "nano/lib/rsnanoutils.hpp"

#include <nano/lib/logger_mt.hpp>
#include <nano/lib/stats.hpp>
#include <nano/node/confirmation_height_bounded.hpp>
#include <nano/node/logging.hpp>
#include <nano/node/write_database_queue.hpp>
#include <nano/secure/ledger.hpp>

#include <boost/format.hpp>

#include <chrono>
#include <iterator>
#include <optional>

nano::hash_circular_buffer::hash_circular_buffer (size_t max_items) :
	handle{ rsnano::rsn_hash_circular_buffer_create (max_items) }
{
}

nano::hash_circular_buffer::~hash_circular_buffer ()
{
	rsnano::rsn_hash_circular_buffer_destroy (handle);
}

bool nano::hash_circular_buffer::empty () const
{
	return rsnano::rsn_hash_circular_buffer_empty (handle);
}

nano::block_hash nano::hash_circular_buffer::back () const
{
	nano::block_hash result;
	rsnano::rsn_hash_circular_buffer_back (handle, result.bytes.data ());
	return result;
}

void nano::hash_circular_buffer::push_back (nano::block_hash const & hash)
{
	rsnano::rsn_hash_circular_buffer_push_back (handle, hash.bytes.data ());
}

void nano::hash_circular_buffer::truncate_after (nano::block_hash const & hash)
{
	rsnano::rsn_hash_circular_buffer_truncate_after (handle, hash.bytes.data ());
}

namespace
{
void notify_observers_callback_wrapper (void * context, rsnano::BlockVecHandle * blocks_handle)
{
	auto callback = static_cast<std::function<void (std::vector<std::shared_ptr<nano::block>> const &)> *> (context);
	rsnano::block_vec block_vec{ blocks_handle };
	auto blocks = block_vec.to_vector ();
	(*callback) (blocks);
}

void notify_observers_delete_context (void * context)
{
	auto callback = static_cast<std::function<void (std::vector<std::shared_ptr<nano::block>> const &)> *> (context);
	delete callback;
}

uint64_t awaiting_processing_size_callback_wrapper (void * context_a)
{
	auto fn = static_cast<std::function<uint64_t ()> *> (context_a);
	return (*fn) ();
}

void drop_awaiting_processing_size_callback (void * context_a)
{
	auto fn = static_cast<std::function<uint64_t ()> *> (context_a);
	delete fn;
}

void block_already_cemented_callback_wrapper (void * context_a, const uint8_t * block_bytes)
{
	auto fn = static_cast<std::function<void (nano::block_hash const &)> *> (context_a);
	auto block{ nano::block_hash::from_bytes (block_bytes) };
	return (*fn) (block);
}

void drop_block_already_cemented_context (void * context_a)
{
	auto fn = static_cast<std::function<void (nano::block_hash const &)> *> (context_a);
	delete fn;
}

rsnano::ConfirmationHeightBoundedHandle * create_conf_height_bounded_handle (
nano::write_database_queue & write_database_queue_a,
std::function<void (std::vector<std::shared_ptr<nano::block>> const &)> const & notify_observers_callback_a,
rsnano::AtomicU64Wrapper & batch_write_size_a,
std::shared_ptr<nano::logger_mt> & logger_a,
nano::logging const & logging_a,
nano::ledger & ledger_a,
rsnano::AtomicBoolWrapper & stopped_a,
std::chrono::milliseconds batch_separate_pending_min_time_a,
std::function<uint64_t ()> const & awaiting_processing_size_callback_a,
std::function<void (nano::block_hash const &)> const & notify_block_already_cemented_observers_callback_a)
{
	auto awaiting_processing_size_context = new std::function<uint64_t ()> (awaiting_processing_size_callback_a);
	auto notify_observers_context = new std::function<void (std::vector<std::shared_ptr<nano::block>> const &)> (notify_observers_callback_a);
	auto block_already_cemented_context = new std::function<void (nano::block_hash const &)> (notify_block_already_cemented_observers_callback_a);

	auto logging_dto{ logging_a.to_dto () };
	return rsnano::rsn_confirmation_height_bounded_create (
	write_database_queue_a.handle,
	notify_observers_callback_wrapper,
	notify_observers_context,
	notify_observers_delete_context,
	batch_write_size_a.handle,
	nano::to_logger_handle (logger_a),
	&logging_dto,
	ledger_a.handle,
	stopped_a.handle,
	batch_separate_pending_min_time_a.count (),
	awaiting_processing_size_callback_wrapper,
	awaiting_processing_size_context,
	drop_awaiting_processing_size_callback,
	block_already_cemented_callback_wrapper,
	block_already_cemented_context,
	drop_block_already_cemented_context);
}
}

nano::confirmation_height_bounded::confirmation_height_bounded (nano::ledger & ledger_a, nano::write_database_queue & write_database_queue_a, std::chrono::milliseconds batch_separate_pending_min_time_a, nano::logging const & logging_a, std::shared_ptr<nano::logger_mt> & logger_a, rsnano::AtomicBoolWrapper & stopped_a, rsnano::AtomicU64Wrapper & batch_write_size_a, std::function<void (std::vector<std::shared_ptr<nano::block>> const &)> const & notify_observers_callback_a, std::function<void (nano::block_hash const &)> const & notify_block_already_cemented_observers_callback_a, std::function<uint64_t ()> const & awaiting_processing_size_callback_a) :
	handle{ create_conf_height_bounded_handle (write_database_queue_a, notify_observers_callback_a, batch_write_size_a, logger_a, logging_a, ledger_a, stopped_a, batch_separate_pending_min_time_a, awaiting_processing_size_callback_a, notify_block_already_cemented_observers_callback_a) },
	ledger (ledger_a),
	write_database_queue (write_database_queue_a),
	batch_separate_pending_min_time (batch_separate_pending_min_time_a),
	logging (logging_a),
	logger (logger_a),
	stopped (stopped_a),
	batch_write_size (batch_write_size_a),
	notify_observers_callback (notify_observers_callback_a),
	notify_block_already_cemented_observers_callback (notify_block_already_cemented_observers_callback_a),
	awaiting_processing_size_callback (awaiting_processing_size_callback_a)
{
}

nano::confirmation_height_bounded::~confirmation_height_bounded ()
{
	rsnano::rsn_confirmation_height_bounded_destroy (handle);
}

void nano::confirmation_height_bounded::process (std::shared_ptr<nano::block> original_block)
{
	rsnano::rsn_confirmation_height_bounded_process (
	handle,
	original_block->get_handle ());
}

void nano::confirmation_height_bounded::cement_blocks (nano::write_guard & scoped_write_guard_a)
{
	auto write_guard_handle = rsnano::rsn_confirmation_height_bounded_cement_blocks (handle, scoped_write_guard_a.handle);

	if (write_guard_handle != nullptr)
	{
		scoped_write_guard_a = nano::write_guard{ write_guard_handle };
	}
}

bool nano::confirmation_height_bounded::pending_empty () const
{
	return rsnano::rsn_confirmation_height_bounded_pending_empty (handle);
}

void nano::confirmation_height_bounded::clear_process_vars ()
{
	rsnano::rsn_confirmation_height_bounded_clear_process_vars (handle);
}

std::unique_ptr<nano::container_info_component> nano::collect_container_info (confirmation_height_bounded & confirmation_height_bounded, std::string const & name_a)
{
	auto composite = std::make_unique<container_info_composite> (name_a);
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "pending_writes", rsnano::rsn_confirmation_height_bounded_pending_writes_size (confirmation_height_bounded.handle), rsnano::rsn_confirmation_height_bounded_write_details_size () }));
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "accounts_confirmed_info", rsnano::rsn_confirmation_height_bounded_accounts_confirmed_info_size (confirmation_height_bounded.handle), rsnano::rsn_confirmation_height_bounded_confirmed_info_entry_size () }));
	return composite;
}
