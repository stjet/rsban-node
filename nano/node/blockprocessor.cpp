#include "nano/lib/blocks.hpp"
#include "nano/lib/logging.hpp"
#include "nano/lib/rsnano.hpp"
#include "nano/lib/rsnanoutils.hpp"

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

	rsnano::BlockProcessorLockHandle * handle;
	nano::block_processor & block_processor;
};
}

namespace
{
void blocks_rolled_back_wrapper (void * context, rsnano::BlockVecHandle * rolled_back, rsnano::BlockHandle * initial_block)
{
	auto callback = static_cast<std::function<void (std::vector<std::shared_ptr<nano::block>> const &, std::shared_ptr<nano::block> const &)> *> (context);
	auto initial = nano::block_handle_to_block (initial_block);
	rsnano::block_vec blocks{ rolled_back };
	auto vec{ blocks.to_vector () };
	(*callback) (vec, initial);
}

void blocks_rolled_back_delete (void * context)
{
	auto callback = static_cast<std::function<void (std::vector<std::shared_ptr<nano::block>> const &, std::shared_ptr<nano::block> const &)> *> (context);
	delete callback;
}
}

nano::block_processor::block_processor (nano::node & node_a, nano::write_database_queue & write_database_queue_a) :
	config (*node_a.config),
	network_params (node_a.network_params),
	flags (node_a.flags),
	block_arrival (node_a.block_arrival),
	stats{ *node_a.stats }
{
	auto config_dto{ config.to_dto () };
	auto logger_handle = nano::to_logger_handle (node_a.logger);
	handle = rsnano::rsn_block_processor_create (
	this,
	&config_dto,
	logger_handle.handle,
	node_a.flags.handle,
	node_a.ledger.handle,
	node_a.unchecked.handle,
	node_a.gap_cache.handle,
	node_a.stats->handle,
	&node_a.config->network_params.work.dto,
	write_database_queue_a.handle);

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

std::size_t nano::block_processor::size ()
{
	nano::block_processor_lock lock{ *this };
	return (lock.blocks_size () + lock.forced_size ());
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

void nano::block_processor::set_blocks_rolled_back_callback (std::function<void (std::vector<std::shared_ptr<nano::block>> const &, std::shared_ptr<nano::block> const &)> callback)
{
	rsnano::rsn_block_processor_set_blocks_rolled_back_callback (
	handle,
	blocks_rolled_back_wrapper,
	new std::function<void (std::vector<std::shared_ptr<nano::block>> const &, std::shared_ptr<nano::block> const &)> (callback),
	blocks_rolled_back_delete);
}

bool nano::block_processor::have_blocks_ready (nano::block_processor_lock & lock_a)
{
	return lock_a.blocks_size () > 0 || lock_a.forced_size () > 0;
}

bool nano::block_processor::have_blocks (nano::block_processor_lock & lock_a)
{
	return have_blocks_ready (lock_a);
}

auto nano::block_processor::process_batch (nano::block_processor_lock & lock_a) -> std::deque<processed_t>
{
	auto result_handle = rsnano::rsn_block_processor_process_batch (handle);
	std::deque<processed_t> result;
	auto size = rsnano::rsn_process_batch_result_size (result_handle);
	for (auto i = 0; i < size; ++i)
	{
		uint8_t result_code = 0;
		auto block_handle = rsnano::rsn_process_batch_result_get (result_handle, i, &result_code);
		auto block = nano::block_handle_to_block (block_handle);
		nano::process_return ret{ static_cast<nano::process_result> (result_code) };
		result.emplace_back (ret, block);
	}
	rsnano::rsn_process_batch_result_destroy (result_handle);
	return result;
}

std::unique_ptr<nano::container_info_component> nano::collect_container_info (block_processor & block_processor, std::string const & name)
{
	auto info_handle = rsnano::rsn_block_processor_collect_container_info (block_processor.handle, name.c_str ());
	return std::make_unique<nano::container_info_composite> (info_handle);
}
