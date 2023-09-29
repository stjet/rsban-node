#include "nano/lib/blocks.hpp"
#include "nano/lib/logger_mt.hpp"
#include "nano/lib/rsnano.hpp"

#include <nano/lib/threading.hpp>
#include <nano/lib/timer.hpp>
#include <nano/node/blockprocessor.hpp>
#include <nano/node/node.hpp>
#include <nano/store/component.hpp>

#include <boost/format.hpp>

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
	handle = rsnano::rsn_block_processor_create (this, &config_dto, checker.get_handle (), config.network_params.ledger.epochs.get_handle (), nano::to_logger_handle (node_a.logger), node_a.flags.handle, node_a.ledger.handle);

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
	nano::process_return result;
	auto hash (block->hash ());
	result = ledger.process (transaction_a, *block);
	switch (result.code)
	{
		case nano::process_result::progress:
		{
			if (config.logging.ledger_logging ())
			{
				std::string block_string;
				block->serialize_json (block_string, config.logging.single_line_record ());
				logger.try_log (boost::str (boost::format ("Processing block %1%: %2%") % hash.to_string () % block_string));
			}
			queue_unchecked (transaction_a, hash);
			/* For send blocks check epoch open unchecked (gap pending).
			For state blocks check only send subtype and only if block epoch is not last epoch.
			If epoch is last, then pending entry shouldn't trigger same epoch open block for destination account. */
			if (block->type () == nano::block_type::send || (block->type () == nano::block_type::state && block->sideband ().details ().is_send () && std::underlying_type_t<nano::epoch> (block->sideband ().details ().epoch ()) < std::underlying_type_t<nano::epoch> (nano::epoch::max)))
			{
				/* block->destination () for legacy send blocks
				block->link () for state blocks (send subtype) */
				queue_unchecked (transaction_a, block->destination ().is_zero () ? block->link () : block->destination ());
			}
			break;
		}
		case nano::process_result::gap_previous:
		{
			if (config.logging.ledger_logging ())
			{
				logger.try_log (boost::str (boost::format ("Gap previous for: %1%") % hash.to_string ()));
			}
			unchecked.put (block->previous (), block);
			stats.inc (nano::stat::type::ledger, nano::stat::detail::gap_previous);
			break;
		}
		case nano::process_result::gap_source:
		{
			if (config.logging.ledger_logging ())
			{
				logger.try_log (boost::str (boost::format ("Gap source for: %1%") % hash.to_string ()));
			}
			unchecked.put (ledger.block_source (transaction_a, *block), block);
			stats.inc (nano::stat::type::ledger, nano::stat::detail::gap_source);
			break;
		}
		case nano::process_result::gap_epoch_open_pending:
		{
			if (config.logging.ledger_logging ())
			{
				logger.try_log (boost::str (boost::format ("Gap pending entries for epoch open: %1%") % hash.to_string ()));
			}
			unchecked.put (block->account (), block); // Specific unchecked key starting with epoch open block account public key
			stats.inc (nano::stat::type::ledger, nano::stat::detail::gap_source);
			break;
		}
		case nano::process_result::old:
		{
			if (config.logging.ledger_duplicate_logging ())
			{
				logger.try_log (boost::str (boost::format ("Old for: %1%") % hash.to_string ()));
			}
			stats.inc (nano::stat::type::ledger, nano::stat::detail::old);
			break;
		}
		case nano::process_result::bad_signature:
		{
			if (config.logging.ledger_logging ())
			{
				logger.try_log (boost::str (boost::format ("Bad signature for: %1%") % hash.to_string ()));
			}
			break;
		}
		case nano::process_result::negative_spend:
		{
			if (config.logging.ledger_logging ())
			{
				logger.try_log (boost::str (boost::format ("Negative spend for: %1%") % hash.to_string ()));
			}
			break;
		}
		case nano::process_result::unreceivable:
		{
			if (config.logging.ledger_logging ())
			{
				logger.try_log (boost::str (boost::format ("Unreceivable for: %1%") % hash.to_string ()));
			}
			break;
		}
		case nano::process_result::fork:
		{
			stats.inc (nano::stat::type::ledger, nano::stat::detail::fork);
			if (config.logging.ledger_logging ())
			{
				logger.try_log (boost::str (boost::format ("Fork for: %1% root: %2%") % hash.to_string () % block->root ().to_string ()));
			}
			break;
		}
		case nano::process_result::opened_burn_account:
		{
			if (config.logging.ledger_logging ())
			{
				logger.try_log (boost::str (boost::format ("Rejecting open block for burn account: %1%") % hash.to_string ()));
			}
			break;
		}
		case nano::process_result::balance_mismatch:
		{
			if (config.logging.ledger_logging ())
			{
				logger.try_log (boost::str (boost::format ("Balance mismatch for: %1%") % hash.to_string ()));
			}
			break;
		}
		case nano::process_result::representative_mismatch:
		{
			if (config.logging.ledger_logging ())
			{
				logger.try_log (boost::str (boost::format ("Representative mismatch for: %1%") % hash.to_string ()));
			}
			break;
		}
		case nano::process_result::block_position:
		{
			if (config.logging.ledger_logging ())
			{
				logger.try_log (boost::str (boost::format ("Block %1% cannot follow predecessor %2%") % hash.to_string () % block->previous ().to_string ()));
			}
			break;
		}
		case nano::process_result::insufficient_work:
		{
			if (config.logging.ledger_logging ())
			{
				logger.try_log (boost::str (boost::format ("Insufficient work for %1% : %2% (difficulty %3%)") % hash.to_string () % nano::to_string_hex (block->block_work ()) % nano::to_string_hex (network_params.work.difficulty (*block))));
			}
			break;
		}
	}

	stats.inc (nano::stat::type::blockprocessor, nano::to_stat_detail (result.code));

	return result;
}

void nano::block_processor::queue_unchecked (store::write_transaction const & transaction_a, nano::hash_or_account const & hash_or_account_a)
{
	unchecked.trigger (hash_or_account_a);
	gap_cache.erase (hash_or_account_a.hash);
}

std::unique_ptr<nano::container_info_component> nano::collect_container_info (block_processor & block_processor, std::string const & name)
{
	std::size_t blocks_count;
	std::size_t forced_count;

	{
		nano::block_processor_lock lock{ block_processor };
		blocks_count = lock.blocks_size ();
		forced_count = lock.forced_size ();
	}

	auto composite = std::make_unique<container_info_composite> (name);
	// TODO enable again:
	//composite->add_component (collect_container_info (block_processor.state_block_signature_verification, "state_block_signature_verification"));
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "blocks", blocks_count, sizeof (std::shared_ptr<nano::block>) }));
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "forced", forced_count, sizeof (std::shared_ptr<nano::block>) }));
	return composite;
}
