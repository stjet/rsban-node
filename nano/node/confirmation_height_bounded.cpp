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

rsnano::ConfirmationHeightBoundedHandle * create_conf_height_bounded_handle (
nano::write_database_queue & write_database_queue_a,
std::function<void (std::vector<std::shared_ptr<nano::block>> const &)> const & notify_observers_callback_a,
rsnano::AtomicU64Wrapper & batch_write_size_a,
std::shared_ptr<nano::logger_mt> & logger_a,
nano::logging const & logging_a,
nano::ledger & ledger_a,
rsnano::AtomicBoolWrapper & stopped_a,
rsnano::RsNanoTimer & timer_a,
std::chrono::milliseconds batch_separate_pending_min_time_a,
std::function<uint64_t ()> const & awaiting_processing_size_callback_a)
{
	auto callback_context = new std::function<uint64_t ()> (awaiting_processing_size_callback_a);
	auto notify_observers_context = new std::function<void (std::vector<std::shared_ptr<nano::block>> const &)> (notify_observers_callback_a);
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
	timer_a.handle,
	batch_separate_pending_min_time_a.count (),
	awaiting_processing_size_callback_wrapper,
	callback_context,
	drop_awaiting_processing_size_callback);
}
}

nano::confirmation_height_bounded::confirmation_height_bounded (nano::ledger & ledger_a, nano::write_database_queue & write_database_queue_a, std::chrono::milliseconds batch_separate_pending_min_time_a, nano::logging const & logging_a, std::shared_ptr<nano::logger_mt> & logger_a, rsnano::AtomicBoolWrapper & stopped_a, rsnano::AtomicU64Wrapper & batch_write_size_a, std::function<void (std::vector<std::shared_ptr<nano::block>> const &)> const & notify_observers_callback_a, std::function<void (nano::block_hash const &)> const & notify_block_already_cemented_observers_callback_a, std::function<uint64_t ()> const & awaiting_processing_size_callback_a) :
	timer{},
	handle{ create_conf_height_bounded_handle (write_database_queue_a, notify_observers_callback_a, batch_write_size_a, logger_a, logging_a, ledger_a, stopped_a, timer, batch_separate_pending_min_time_a, awaiting_processing_size_callback_a) },
	accounts_confirmed_info{ handle },
	pending_writes{ handle },
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

// The next block hash to iterate over, the priority is as follows:
// 1 - The next block in the account chain for the last processed receive (if there is any)
// 2 - The next receive block which is closest to genesis
// 3 - The last checkpoint hit.
// 4 - The hash that was passed in originally. Either all checkpoints were exhausted (this can happen when there are many accounts to genesis)
//     or all other blocks have been processed.
nano::confirmation_height_bounded::top_and_next_hash nano::confirmation_height_bounded::get_next_block (
boost::optional<top_and_next_hash> const & next_in_receive_chain_a,
nano::hash_circular_buffer const & checkpoints_a,
receive_source_pair_circular_buffer const & receive_source_pairs,
boost::optional<receive_chain_details> & receive_details_a,
nano::block const & original_block)
{
	rsnano::TopAndNextHashDto next_in_chain_dto{};
	if (next_in_receive_chain_a)
	{
		next_in_chain_dto = next_in_receive_chain_a->to_dto ();
	}

	rsnano::ReceiveChainDetailsDto receive_details_dto{};
	if (receive_details_a)
	{
		receive_details_dto = receive_details_a->to_dto ();
	}
	bool has_receive_details = receive_details_a.is_initialized ();

	rsnano::TopAndNextHashDto next_dto{};

	rsnano::rsn_confirmation_height_bounded_get_next_block (
	handle,
	&next_in_chain_dto,
	next_in_receive_chain_a.is_initialized (),
	checkpoints_a.handle,
	receive_source_pairs.handle,
	&receive_details_dto,
	&has_receive_details,
	original_block.get_handle (),
	&next_dto);

	if (has_receive_details)
	{
		receive_details_a = receive_chain_details{ receive_details_dto };
	}
	else
	{
		receive_details_a = boost::none;
	}

	top_and_next_hash next{ next_dto };
	return next;
}

void nano::confirmation_height_bounded::process (std::shared_ptr<nano::block> original_block)
{
	if (pending_empty ())
	{
		clear_process_vars ();
		timer.restart ();
	}

	boost::optional<top_and_next_hash> next_in_receive_chain;
	nano::hash_circular_buffer checkpoints{ max_items };
	receive_source_pair_circular_buffer receive_source_pairs{ max_items };
	nano::block_hash current;
	bool first_iter = true;
	auto transaction (ledger.store.tx_begin_read ());
	do
	{
		boost::optional<receive_chain_details> receive_details;
		auto hash_to_process = get_next_block (next_in_receive_chain, checkpoints, receive_source_pairs, receive_details, *original_block);
		current = hash_to_process.top;

		auto top_level_hash = current;
		std::shared_ptr<nano::block> block;
		if (first_iter)
		{
			debug_assert (current == original_block->hash ());
			block = original_block;
		}
		else
		{
			block = ledger.store.block ().get (*transaction, current);
		}

		if (!block)
		{
			if (ledger.pruning_enabled () && ledger.store.pruned ().exists (*transaction, current))
			{
				if (!receive_source_pairs.empty ())
				{
					receive_source_pairs.pop_back ();
				}
				continue;
			}
			else
			{
				auto error_str = (boost::format ("Ledger mismatch trying to set confirmation height for block %1% (bounded processor)") % current.to_string ()).str ();
				logger->always_log (error_str);
				std::cerr << error_str << std::endl;
				release_assert (block);
			}
		}
		nano::account account (block->account ());
		if (account.is_zero ())
		{
			account = block->sideband ().account ();
		}

		// Checks if we have encountered this account before but not commited changes yet, if so then update the cached confirmation height
		nano::confirmation_height_info confirmation_height_info;
		auto found_info = accounts_confirmed_info.find (account);
		if (found_info)
		{
			confirmation_height_info = nano::confirmation_height_info (found_info->confirmed_height, found_info->iterated_frontier);
		}
		else
		{
			ledger.store.confirmation_height ().get (*transaction, account, confirmation_height_info);
			// This block was added to the confirmation height processor but is already confirmed
			if (first_iter && confirmation_height_info.height () >= block->sideband ().height () && current == original_block->hash ())
			{
				notify_block_already_cemented_observers_callback (original_block->hash ());
			}
		}

		auto block_height = block->sideband ().height ();
		bool already_cemented = confirmation_height_info.height () >= block_height;

		// If we are not already at the bottom of the account chain (1 above cemented frontier) then find it
		if (!already_cemented && block_height - confirmation_height_info.height () > 1)
		{
			if (block_height - confirmation_height_info.height () == 2)
			{
				// If there is 1 uncemented block in-between this block and the cemented frontier,
				// we can just use the previous block to get the least unconfirmed hash.
				current = block->previous ();
				--block_height;
			}
			else if (!next_in_receive_chain.is_initialized ())
			{
				current = get_least_unconfirmed_hash_from_top_level (*transaction, current, account, confirmation_height_info, block_height);
			}
			else
			{
				// Use the cached successor of the last receive which saves having to do more IO in get_least_unconfirmed_hash_from_top_level
				// as we already know what the next block we should process should be.
				current = *hash_to_process.next;
				block_height = hash_to_process.next_height;
			}
		}

		auto top_most_non_receive_block_hash = current;

		bool hit_receive = false;
		if (!already_cemented)
		{
			hit_receive = iterate (*transaction, block_height, current, checkpoints, top_most_non_receive_block_hash, top_level_hash, receive_source_pairs, account);
		}

		// Call into Rust...
		//----------------------------------------
		rsnano::TopAndNextHashDto next_in_receive_chain_dto{};
		bool has_next_in_receive_chain = false;

		bool has_receive_details = receive_details.is_initialized ();
		rsnano::ReceiveChainDetailsDto receive_details_dto{};
		if (receive_details)
		{
			receive_details_dto = receive_details->to_dto ();
		}

		bool should_break = rsnano::rsn_confirmation_height_bounded_process (
		handle,
		current.bytes.data (),
		original_block->get_handle (),
		receive_source_pairs.handle,
		&next_in_receive_chain_dto,
		&has_next_in_receive_chain,
		transaction->get_rust_handle (),
		top_most_non_receive_block_hash.bytes.data (),
		already_cemented,
		checkpoints.handle,
		&confirmation_height_info.dto,
		account.bytes.data (),
		block_height,
		has_receive_details,
		&receive_details_dto,
		hit_receive,
		&first_iter);

		if (has_next_in_receive_chain)
		{
			next_in_receive_chain = top_and_next_hash{ next_in_receive_chain_dto };
		}
		else
		{
			next_in_receive_chain = boost::none;
		}

		if (should_break)
		{
			break;
		}
		//----------------------------------------
	} while ((!receive_source_pairs.empty () || current != original_block->hash ()) && !stopped.load ());

	debug_assert (checkpoints.empty ());
}

nano::block_hash nano::confirmation_height_bounded::get_least_unconfirmed_hash_from_top_level (nano::transaction const & transaction_a, nano::block_hash const & hash_a, nano::account const & account_a, nano::confirmation_height_info const & confirmation_height_info_a, uint64_t & block_height_a)
{
	nano::block_hash least_unconfirmed_hash;
	rsnano::rsn_confirmation_height_bounded_get_least_unconfirmed_hash_from_top_level (
	handle,
	transaction_a.get_rust_handle (),
	hash_a.bytes.data (),
	account_a.bytes.data (),
	&confirmation_height_info_a.dto,
	&block_height_a,
	least_unconfirmed_hash.bytes.data ());
	return least_unconfirmed_hash;
}

bool nano::confirmation_height_bounded::iterate (
nano::read_transaction & transaction_a,
uint64_t bottom_height_a,
nano::block_hash const & bottom_hash_a,
nano::hash_circular_buffer & checkpoints_a,
nano::block_hash & top_most_non_receive_block_hash_a,
nano::block_hash const & top_level_hash_a,
receive_source_pair_circular_buffer & receive_source_pairs_a,
nano::account const & account_a)
{
	bool hit_receive = rsnano::rsn_confirmation_height_bounded_iterate (
	handle,
	receive_source_pairs_a.handle,
	checkpoints_a.handle,
	top_level_hash_a.bytes.data (),
	account_a.bytes.data (),
	bottom_height_a,
	bottom_hash_a.bytes.data (),
	top_most_non_receive_block_hash_a.bytes.data (),
	transaction_a.get_rust_handle ());

	return hit_receive;
}

// Once the path to genesis has been iterated to, we can begin to cement the lowest blocks in the accounts. This sets up
// the non-receive blocks which have been iterated for an account, and the associated receive block.
boost::optional<nano::confirmation_height_bounded::top_and_next_hash> nano::confirmation_height_bounded::prepare_iterated_blocks_for_cementing (preparation_data & preparation_data_a)
{
	rsnano::ReceiveChainDetailsDto details_dto;
	auto & receive_details = preparation_data_a.receive_details;
	if (receive_details)
	{
		details_dto = receive_details->to_dto ();
	}
	bool has_next_dto = preparation_data_a.next_in_receive_chain.has_value ();
	rsnano::TopAndNextHashDto next_dto;
	if (preparation_data_a.next_in_receive_chain)
	{
		next_dto = preparation_data_a.next_in_receive_chain->to_dto ();
	}

	rsnano::rsn_confirmation_height_bounded_prepare_iterated_blocks_for_cementing (
	handle,
	receive_details.is_initialized (),
	&details_dto,
	preparation_data_a.checkpoints.handle,
	&has_next_dto,
	&next_dto,
	preparation_data_a.already_cemented,
	preparation_data_a.transaction.get_rust_handle (),
	preparation_data_a.top_most_non_receive_block_hash.bytes.data (),
	&preparation_data_a.confirmation_height_info.dto,
	preparation_data_a.account.bytes.data (),
	preparation_data_a.bottom_height,
	preparation_data_a.bottom_most.bytes.data ());

	boost::optional<top_and_next_hash> next_in_receive_chain;
	if (has_next_dto)
	{
		next_in_receive_chain = nano::confirmation_height_bounded::top_and_next_hash{ next_dto };
	}
	else
	{
		next_in_receive_chain = boost::none;
	}
	return next_in_receive_chain;
}

void nano::confirmation_height_bounded::cement_blocks (nano::write_guard & scoped_write_guard_a)
{
	auto write_guard_handle = rsnano::rsn_confirmation_height_bounded_cement_blocks (handle, scoped_write_guard_a.handle);

	if (write_guard_handle != nullptr)
	{
		scoped_write_guard_a = nano::write_guard{ write_guard_handle };
	}

	timer.restart ();
}

bool nano::confirmation_height_bounded::pending_empty () const
{
	return pending_writes.empty ();
}

void nano::confirmation_height_bounded::clear_process_vars ()
{
	accounts_confirmed_info.clear ();
	rsnano::rsn_confirmation_height_bounded_accounts_confirmed_info_size_store (handle, 0);
}

nano::confirmation_height_bounded::receive_chain_details::receive_chain_details (nano::account const & account_a, uint64_t height_a, nano::block_hash const & hash_a, nano::block_hash const & top_level_a, boost::optional<nano::block_hash> next_a, uint64_t bottom_height_a, nano::block_hash const & bottom_most_a) :
	account (account_a),
	height (height_a),
	hash (hash_a),
	top_level (top_level_a),
	next (next_a),
	bottom_height (bottom_height_a),
	bottom_most (bottom_most_a)
{
}

nano::confirmation_height_bounded::receive_chain_details::receive_chain_details (rsnano::ReceiveChainDetailsDto const & dto) :
	account (nano::account::from_bytes (dto.account)),
	height (dto.height),
	hash (nano::block_hash::from_bytes (dto.hash)),
	top_level (nano::block_hash::from_bytes (dto.top_level)),
	next (boost::none),
	bottom_height (dto.bottom_height),
	bottom_most (nano::block_hash::from_bytes (dto.bottom_most))
{
	if (dto.has_next)
	{
		next = nano::block_hash::from_bytes (dto.next);
	}
}

nano::confirmation_height_bounded::write_details::write_details (nano::account const & account_a, uint64_t bottom_height_a, nano::block_hash const & bottom_hash_a, uint64_t top_height_a, nano::block_hash const & top_hash_a) :
	account (account_a),
	bottom_height (bottom_height_a),
	bottom_hash (bottom_hash_a),
	top_height (top_height_a),
	top_hash (top_hash_a)
{
}

nano::confirmation_height_bounded::write_details::write_details (rsnano::WriteDetailsDto const & dto) :
	bottom_height (dto.bottom_height),
	top_height (dto.top_height)
{
	std::copy (std::begin (dto.account), std::end (dto.account), std::begin (account.bytes));
	std::copy (std::begin (dto.bottom_hash), std::end (dto.bottom_hash), std::begin (bottom_hash.bytes));
	std::copy (std::begin (dto.top_hash), std::end (dto.top_hash), std::begin (top_hash.bytes));
}

rsnano::WriteDetailsDto nano::confirmation_height_bounded::write_details::to_dto () const
{
	rsnano::WriteDetailsDto dto;
	std::copy (std::begin (account.bytes), std::end (account.bytes), std::begin (dto.account));
	std::copy (std::begin (bottom_hash.bytes), std::end (bottom_hash.bytes), std::begin (dto.bottom_hash));
	std::copy (std::begin (top_hash.bytes), std::end (top_hash.bytes), std::begin (dto.top_hash));
	dto.bottom_height = bottom_height;
	dto.top_height = top_height;
	return dto;
}

nano::confirmation_height_bounded::receive_source_pair::receive_source_pair (confirmation_height_bounded::receive_chain_details const & receive_details_a, const block_hash & source_a) :
	receive_details (receive_details_a),
	source_hash (source_a)
{
}

nano::confirmation_height_bounded::receive_source_pair::receive_source_pair (rsnano::ReceiveSourcePairDto const & pair_dto) :
	receive_details{ pair_dto.receive_details },
	source_hash{ nano::block_hash::from_bytes (pair_dto.source_hash) }
{
}

nano::confirmation_height_bounded::confirmed_info::confirmed_info (uint64_t confirmed_height_a, nano::block_hash const & iterated_frontier_a) :
	confirmed_height (confirmed_height_a),
	iterated_frontier (iterated_frontier_a)
{
}

std::unique_ptr<nano::container_info_component> nano::collect_container_info (confirmation_height_bounded & confirmation_height_bounded, std::string const & name_a)
{
	auto composite = std::make_unique<container_info_composite> (name_a);
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "pending_writes", rsnano::rsn_confirmation_height_bounded_pending_writes_size (confirmation_height_bounded.handle), sizeof (nano::confirmation_height_bounded::write_details) }));
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "accounts_confirmed_info", rsnano::rsn_confirmation_height_bounded_accounts_confirmed_info_size (confirmation_height_bounded.handle), sizeof (nano::account) + sizeof (nano::confirmation_height_bounded::confirmed_info) }));
	return composite;
}

nano::confirmation_height_bounded::pending_writes_queue::pending_writes_queue (rsnano::ConfirmationHeightBoundedHandle * handle_a) :
	handle{ handle_a }
{
}

size_t nano::confirmation_height_bounded::pending_writes_queue::size () const
{
	return rsnano::rsn_pending_writes_queue_size (handle);
}

bool nano::confirmation_height_bounded::pending_writes_queue::empty () const
{
	return size () == 0;
}

void nano::confirmation_height_bounded::pending_writes_queue::push_back (nano::confirmation_height_bounded::write_details const & details)
{
	auto dto{ details.to_dto () };
	rsnano::rsn_pending_writes_queue_push_back (handle, &dto);
}

nano::confirmation_height_bounded::write_details nano::confirmation_height_bounded::pending_writes_queue::front () const
{
	rsnano::WriteDetailsDto details_dto;
	rsnano::rsn_pending_writes_queue_front (handle, &details_dto);
	return nano::confirmation_height_bounded::write_details{ details_dto };
}

void nano::confirmation_height_bounded::pending_writes_queue::pop_front ()
{
	rsnano::rsn_pending_writes_queue_pop_front (handle);
}

uint64_t nano::confirmation_height_bounded::pending_writes_queue::total_pending_write_block_count () const
{
	return rsnano::rsn_pending_writes_queue_total_pending_write_block_count (handle);
}

rsnano::ReceiveChainDetailsDto nano::confirmation_height_bounded::receive_chain_details::to_dto () const
{
	rsnano::ReceiveChainDetailsDto dto;
	account.copy_bytes_to (dto.account);
	dto.height = height;
	hash.copy_bytes_to (dto.hash);
	top_level.copy_bytes_to (dto.top_level);
	dto.has_next = next.has_value ();
	if (next)
	{
		next->copy_bytes_to (dto.next);
	}
	dto.bottom_height = bottom_height;
	bottom_most.copy_bytes_to (dto.bottom_most);
	return dto;
}

rsnano::ReceiveSourcePairDto nano::confirmation_height_bounded::receive_source_pair::to_dto () const
{
	rsnano::ReceiveSourcePairDto dto;
	source_hash.copy_bytes_to (dto.source_hash);
	dto.receive_details = receive_details.to_dto ();
	return dto;
}

nano::confirmation_height_bounded::receive_source_pair_circular_buffer::receive_source_pair_circular_buffer (size_t max_items) :
	handle{ rsnano::rsn_receive_source_pair_circular_buffer_create (max_items) }
{
}

nano::confirmation_height_bounded::receive_source_pair_circular_buffer::~receive_source_pair_circular_buffer ()
{
	rsnano::rsn_receive_source_pair_circular_buffer_destroy (handle);
}

void nano::confirmation_height_bounded::receive_source_pair_circular_buffer::push_back (nano::confirmation_height_bounded::receive_source_pair const & pair)
{
	auto pair_dto{ pair.to_dto () };
	rsnano::rsn_receive_source_pair_circular_buffer_push_back (handle, &pair_dto);
}

bool nano::confirmation_height_bounded::receive_source_pair_circular_buffer::empty () const
{
	return rsnano::rsn_receive_source_pair_circular_buffer_size (handle) == 0;
}

size_t nano::confirmation_height_bounded::receive_source_pair_circular_buffer::size () const
{
	return rsnano::rsn_receive_source_pair_circular_buffer_size (handle);
}

nano::confirmation_height_bounded::receive_source_pair nano::confirmation_height_bounded::receive_source_pair_circular_buffer::back () const
{
	rsnano::ReceiveSourcePairDto pair_dto;
	rsnano::rsn_receive_source_pair_circular_buffer_back (handle, &pair_dto);
	return nano::confirmation_height_bounded::receive_source_pair{ pair_dto };
}

void nano::confirmation_height_bounded::receive_source_pair_circular_buffer::pop_back ()
{
	rsnano::rsn_receive_source_pair_circular_buffer_pop_back (handle);
}