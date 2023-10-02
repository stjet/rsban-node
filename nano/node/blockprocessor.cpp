#include "nano/lib/blocks.hpp"
#include "nano/lib/logger_mt.hpp"
#include "nano/lib/rsnano.hpp"

#include <nano/lib/threading.hpp>
#include <nano/lib/timer.hpp>
#include <nano/node/blockprocessor.hpp>
#include <nano/node/node.hpp>
#include <nano/store/component.hpp>

#include <boost/format.hpp>

#include <cstdint>
#include <memory>

namespace nano
{
class block_processor_lock
{
public:
	block_processor_lock (nano::block_processor & block_processor_a) :
		handle{ rsnano::rsn_block_processor_lock (block_processor_a.handle) },
		block_processor{ block_processor_a }
	{
	}

	block_processor_lock (block_processor_lock const &) = delete;

	~block_processor_lock ()
	{
		rsnano::rsn_block_processor_lock_destroy (handle);
	}

	void lock (rsnano::BlockProcessorHandle * processor)
	{
		rsnano::rsn_block_processor_lock_lock (handle, processor);
	}

	void unlock ()
	{
		rsnano::rsn_block_processor_lock_unlock (handle);
	}

	void push_back_block (std::shared_ptr<nano::block> const & block)
	{
		rsnano::rsn_block_processor_push_back_block (handle, block->get_handle ());
	}

	void push_back_forced (std::shared_ptr<nano::block> const & block)
	{
		rsnano::rsn_block_processor_push_back_forced (handle, block->get_handle ());
	}

	std::shared_ptr<nano::block> pop_front_block ()
	{
		auto block_handle = rsnano::rsn_block_processor_pop_front_block (handle);
		return nano::block_handle_to_block (block_handle);
	}

	std::shared_ptr<nano::block> pop_front_forced ()
	{
		auto block_handle = rsnano::rsn_block_processor_pop_front_forced (handle);
		return nano::block_handle_to_block (block_handle);
	}

	std::size_t blocks_size () const
	{
		return rsnano::rsn_block_processor_blocks_size (handle);
	}

	std::size_t forced_size () const
	{
		return rsnano::rsn_block_processor_forced_size (handle);
	}

	bool should_log ()
	{
		return rsnano::rsn_block_processor_should_log (handle);
	}

	rsnano::BlockProcessorLockHandle * handle;
	nano::block_processor & block_processor;
};
}

nano::block_processor::block_processor (nano::node & node_a, nano::write_database_queue & write_database_queue_a) :
	logger (*node_a.logger),
	checker (node_a.checker),
	config (*node_a.config),
	network_params (node_a.network_params),
	ledger (node_a.ledger),
	flags (node_a.flags),
	store (node_a.store),
	stats (*node_a.stats),
	block_arrival (node_a.block_arrival),
	unchecked (node_a.unchecked),
	gap_cache (node_a.gap_cache),
	write_database_queue (write_database_queue_a)
{
	blocks_rolled_back =
	[&node_a] (std::vector<std::shared_ptr<nano::block>> const & rolled_back, std::shared_ptr<nano::block> const & initial_block) {
		// Deleting from votes cache, stop active transaction
		for (auto & i : rolled_back)
		{
			node_a.history.erase (i->root ());
			// Stop all rolled back active transactions except initial
			if (i->hash () != initial_block->hash ())
			{
				node_a.active.erase (*i);
			}
		}
	};

	auto config_dto{ config.to_dto () };
	auto logger_handle = nano::to_logger_handle (node_a.logger);
	handle = rsnano::rsn_block_processor_create (
	this,
	&config_dto,
	checker.get_handle (),
	config.network_params.ledger.epochs.get_handle (),
	logger_handle,
	node_a.flags.handle,
	node_a.ledger.handle,
	node_a.unchecked.handle,
	node_a.gap_cache.handle,
	node_a.stats->handle,
	&node_a.config->network_params.work.dto,
	write_database_queue_a.handle,
	node_a.history.handle);

	batch_processed.add ([this] (auto const & items) {
		// For every batch item: notify the 'processed' observer.
		for (auto const & item : items)
		{
			auto const & [result, block] = item;
			processed.notify (result, block);
		}
	});
	blocking.connect (*this);
}

nano::block_processor::~block_processor ()
{
	rsnano::rsn_block_processor_destroy (handle);
}

rsnano::BlockProcessorHandle const * nano::block_processor::get_handle () const
{
	return handle;
}

void nano::block_processor::start ()
{
	processing_thread = std::thread ([this] () {
		nano::thread_role::set (nano::thread_role::name::block_processing);
		this->process_blocks ();
	});
}

void nano::block_processor::stop ()
{
	{
		nano::block_processor_lock lock{ *this };
		stopped = true;
	}
	rsnano::rsn_block_processor_notify_all (handle);
	blocking.stop ();
	rsnano::rsn_block_processor_stop (handle);
	nano::join_or_pass (processing_thread);
}

void nano::block_processor::flush ()
{
	checker.flush ();
	rsnano::rsn_block_processor_set_flushing (handle, true);
	nano::block_processor_lock lock{ *this };
	while (!stopped && (have_blocks (lock) || active || rsnano::rsn_block_processor_is_signature_verifier_active (handle)))
	{
		rsnano::rsn_block_processor_wait (handle, lock.handle);
	}
	rsnano::rsn_block_processor_set_flushing (handle, false);
}

std::size_t nano::block_processor::size ()
{
	nano::block_processor_lock lock{ *this };
	return (lock.blocks_size () + rsnano::rsn_block_processor_signature_verifier_size (handle) + lock.forced_size ());
}

bool nano::block_processor::full ()
{
	return size () >= flags.block_processor_full_size ();
}

bool nano::block_processor::half_full ()
{
	return size () >= flags.block_processor_full_size () / 2;
}

void nano::block_processor::process_active (std::shared_ptr<nano::block> const & incoming)
{
	block_arrival.add (incoming->hash ());
	add (incoming);
}

void nano::block_processor::add (std::shared_ptr<nano::block> const & block)
{
	if (full ())
	{
		stats.inc (nano::stat::type::blockprocessor, nano::stat::detail::overfill);
		return;
	}
	if (network_params.work.validate_entry (*block)) // true => error
	{
		stats.inc (nano::stat::type::blockprocessor, nano::stat::detail::insufficient_work);
		return;
	}
	rsnano::rsn_block_processor_add_impl (handle, block->get_handle ());
	return;
}

std::optional<nano::process_return> nano::block_processor::add_blocking (std::shared_ptr<nano::block> const & block)
{
	auto future = blocking.insert (block);
	rsnano::rsn_block_processor_add_impl (handle, block->get_handle ());
	rsnano::rsn_block_processor_notify_all (handle);
	std::optional<nano::process_return> result;
	try
	{
		auto status = future.wait_for (config.block_process_timeout);
		debug_assert (status != std::future_status::deferred);
		if (status == std::future_status::ready)
		{
			result = future.get ();
		}
		else
		{
			blocking.erase (block);
		}
	}
	catch (std::future_error const &)
	{
	}
	return result;
}

void nano::block_processor::rollback_competitor (store::write_transaction const & transaction, nano::block const & block)
{
	auto hash = block.hash ();
	auto successor = ledger.successor (transaction, block.qualified_root ());
	if (successor != nullptr && successor->hash () != hash)
	{
		// Replace our block with the winner and roll back any dependent blocks
		if (config.logging.ledger_rollback_logging ())
		{
			logger.always_log (boost::str (boost::format ("Rolling back %1% and replacing with %2%") % successor->hash ().to_string () % hash.to_string ()));
		}
		std::vector<std::shared_ptr<nano::block>> rollback_list;
		if (ledger.rollback (transaction, successor->hash (), rollback_list))
		{
			stats.inc (nano::stat::type::ledger, nano::stat::detail::rollback_failed);
			logger.always_log (nano::severity_level::error, boost::str (boost::format ("Failed to roll back %1% because it or a successor was confirmed") % successor->hash ().to_string ()));
		}
		else if (config.logging.ledger_rollback_logging ())
		{
			logger.always_log (boost::str (boost::format ("%1% blocks rolled back") % rollback_list.size ()));
		}
		blocks_rolled_back (rollback_list, successor);
	}
}

void nano::block_processor::force (std::shared_ptr<nano::block> const & block_a)
{
	{
		nano::block_processor_lock lock{ *this };
		lock.push_back_forced (block_a);
	}
	rsnano::rsn_block_processor_notify_all (handle);
}

void nano::block_processor::process_blocks ()
{
	nano::block_processor_lock lock{ *this };
	while (!stopped)
	{
		if (have_blocks_ready (lock))
		{
			active = true;
			lock.unlock ();
			auto processed = process_batch (lock);
			batch_processed.notify (processed);
			lock.lock (handle);
			active = false;
		}
		else
		{
			rsnano::rsn_block_processor_notify_one (handle);
			rsnano::rsn_block_processor_wait (handle, lock.handle);
		}
	}
}

bool nano::block_processor::flushing ()
{
	return rsnano::rsn_block_processor_flushing (handle);
}

bool nano::block_processor::have_blocks_ready (nano::block_processor_lock & lock_a)
{
	return lock_a.blocks_size () > 0 || lock_a.forced_size () > 0;
}

bool nano::block_processor::have_blocks (nano::block_processor_lock & lock_a)
{
	return have_blocks_ready (lock_a) || rsnano::rsn_block_processor_signature_verifier_size (handle) != 0;
}

auto nano::block_processor::process_batch (nano::block_processor_lock & lock_a) -> std::deque<processed_t>
{
	//TODO enable the Rust port:
	//auto result_handle = rsnano::rsn_block_processor_process_batch (handle);
	//std::deque<processed_t> result;
	//auto size = rsnano::rsn_process_batch_result_size(result_handle);
	//for (auto i = 0; i < size; ++i) {
	//	uint8_t result_code = 0;
	//	nano::process_return ret{static_cast<nano::process_result>(result_code)};
	//	auto block_handle = rsnano::rsn_process_batch_result_get(result_handle, i, &result_code);
	//	auto block = nano::block_handle_to_block(block_handle);
	//	result.emplace_back(ret, block);
	//}
	//rsnano::rsn_process_batch_result_destroy(result_handle);
	//return result;

	std::deque<processed_t> processed;
	auto scoped_write_guard = write_database_queue.wait (nano::writer::process_batch);
	auto transaction (store.tx_begin_write ({ tables::accounts, tables::blocks, tables::frontiers, tables::pending }));
	nano::timer<std::chrono::milliseconds> timer_l;
	lock_a.lock (handle);
	timer_l.start ();
	// Processing blocks
	unsigned number_of_blocks_processed (0), number_of_forced_processed (0);
	auto deadline_reached = [&timer_l, deadline = config.block_processor_batch_max_time] { return timer_l.after_deadline (deadline); };
	auto processor_batch_reached = [&number_of_blocks_processed, max = flags.block_processor_batch_size ()] { return number_of_blocks_processed >= max; };
	auto store_batch_reached = [&number_of_blocks_processed, max = store.max_block_write_batch_num ()] { return number_of_blocks_processed >= max; };
	while (have_blocks_ready (lock_a) && (!deadline_reached () || !processor_batch_reached ()) && !store_batch_reached ())
	{
		if ((lock_a.blocks_size () + rsnano::rsn_block_processor_signature_verifier_size (handle) + lock_a.forced_size () > 64) && lock_a.should_log ())
		{
			logger.always_log (boost::str (boost::format ("%1% blocks (+ %2% state blocks) (+ %3% forced) in processing queue") % lock_a.blocks_size () % rsnano::rsn_block_processor_signature_verifier_size (handle) % lock_a.forced_size ()));
		}
		std::shared_ptr<nano::block> block;
		nano::block_hash hash (0);
		bool force (false);
		if (lock_a.forced_size () == 0)
		{
			block = lock_a.pop_front_block ();
			hash = block->hash ();
		}
		else
		{
			block = lock_a.pop_front_forced ();
			hash = block->hash ();
			force = true;
			number_of_forced_processed++;
		}
		lock_a.unlock ();
		if (force)
		{
			rollback_competitor (*transaction, *block);
		}
		number_of_blocks_processed++;
		auto result = process_one (*transaction, block, force);
		processed.emplace_back (result, block);
		lock_a.lock (handle);
	}
	lock_a.unlock ();

	if (config.logging.timing_logging () && number_of_blocks_processed != 0 && timer_l.stop () > std::chrono::milliseconds (100))
	{
		logger.always_log (boost::str (boost::format ("Processed %1% blocks (%2% blocks were forced) in %3% %4%") % number_of_blocks_processed % number_of_forced_processed % timer_l.value ().count () % timer_l.unit ()));
	}
	return processed;
}

nano::process_return nano::block_processor::process_one (store::write_transaction const & transaction_a, std::shared_ptr<nano::block> block, bool const forced_a)
{
	auto result = rsnano::rsn_block_processor_process_one (handle, transaction_a.get_rust_handle (), block->get_handle ());
	return nano::process_return{ static_cast<nano::process_result> (result) };
}

void nano::block_processor::queue_unchecked (nano::hash_or_account const & hash_or_account_a)
{
	rsnano::rsn_block_processor_queue_unchecked (handle, hash_or_account_a.bytes.data ());
}

std::unique_ptr<nano::container_info_component> nano::collect_container_info (block_processor & block_processor, std::string const & name)
{
	auto info_handle = rsnano::rsn_block_processor_collect_container_info (block_processor.handle, name.c_str ());
	return std::make_unique<nano::container_info_composite> (info_handle);
}
