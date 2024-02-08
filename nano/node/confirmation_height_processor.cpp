#include "nano/lib/blocks.hpp"
#include "nano/lib/rsnano.hpp"
#include "nano/lib/rsnanoutils.hpp"

#include <nano/lib/logging.hpp>
#include <nano/lib/numbers.hpp>
#include <nano/lib/thread_roles.hpp>
#include <nano/lib/utility.hpp>
#include <nano/node/confirmation_height_processor.hpp>
#include <nano/node/write_database_queue.hpp>
#include <nano/secure/common.hpp>
#include <nano/secure/ledger.hpp>

#include <boost/thread/latch.hpp>

#include <cstdint>
#include <memory>

namespace
{
rsnano::ConfirmationHeightProcessorHandle * create_processor_handle (
nano::write_database_queue & write_database_queue_a,
nano::ledger & ledger_a,
std::chrono::milliseconds batch_separate_pending_min_time_a,
boost::latch & latch)
{
	return rsnano::rsn_confirmation_height_processor_create_v2 (
	write_database_queue_a.handle,
	ledger_a.handle,
	batch_separate_pending_min_time_a.count (),
	&latch);
}
}

nano::confirmation_height_processor::confirmation_height_processor (nano::ledger & ledger_a, nano::stats & stats_a, nano::write_database_queue & write_database_queue_a, std::chrono::milliseconds batch_separate_pending_min_time_a, boost::latch & latch) :
	handle{ create_processor_handle (write_database_queue_a, ledger_a, batch_separate_pending_min_time_a, latch) }
{
}

nano::confirmation_height_processor::~confirmation_height_processor ()
{
	rsnano::rsn_confirmation_height_processor_destroy (handle);
}

void nano::confirmation_height_processor::stop ()
{
	rsnano::rsn_confirmation_height_processor_stop (handle);
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
	return rsnano::rsn_confirmation_height_processor_is_processing_block (handle, hash_a.bytes.data ());
}

nano::block_hash nano::confirmation_height_processor::current () const
{
	nano::block_hash hash;
	rsnano::rsn_confirmation_height_processor_current (handle, hash.bytes.data ());
	return hash;
}

void nano::confirmation_height_processor::set_batch_write_size (size_t write_size)
{
	rsnano::rsn_confirmation_height_processor_set_batch_write_size (handle, write_size);
}

std::unique_ptr<nano::container_info_component> nano::confirmation_height_processor::collect_container_info (std::string const & name_a)
{
	return std::make_unique<nano::container_info_composite> (
	rsnano::rsn_confirmation_height_processor_collect_container_info (handle, name_a.c_str ()));
}
