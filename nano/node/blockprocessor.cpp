#include "nano/lib/blocks.hpp"
#include "nano/lib/logging.hpp"
#include "nano/lib/rsnano.hpp"
#include "nano/lib/rsnanoutils.hpp"

#include <nano/lib/threading.hpp>
#include <nano/lib/timer.hpp>
#include <nano/node/active_transactions.hpp>
#include <nano/node/blockprocessor.hpp>
#include <nano/node/local_vote_history.hpp>
#include <nano/node/node.hpp>
#include <nano/secure/ledger.hpp>
#include <nano/store/component.hpp>

#include <boost/format.hpp>

#include <cstdint>
#include <memory>

#include <magic_enum.hpp>

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

	bool queue_empty ()
	{
		return rsnano::rsn_block_processor_lock_queue_empty (handle);
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

void block_rolled_back_wrapper (void * context, rsnano::BlockHandle * block_handle)
{
	auto callback = static_cast<std::function<void (std::shared_ptr<nano::block> const &)> *> (context);
	auto block{ nano::block_handle_to_block (block_handle) };
	(*callback) (block);
}

void block_rolled_back_delete (void * context)
{
	auto callback = static_cast<std::function<void (std::shared_ptr<nano::block> const &)> *> (context);
	delete callback;
}
}

/*
 * block_processor::context
 */

nano::block_processor::context::context (std::shared_ptr<nano::block> block, nano::block_source source_a) :
	source{ source_a },
	handle{ rsnano::rsn_block_processor_context_create (block->get_handle (), static_cast<uint8_t> (source_a)) }
{
	debug_assert (source != nano::block_source::unknown);
}

nano::block_processor::context::context (rsnano::BlockProcessorContextHandle * handle_a) :
	source{ rsnano::rsn_block_processor_context_source (handle_a) },
	handle{ handle_a }
{
}

nano::block_processor::context::context (nano::block_processor::context && other) :
	source{ other.source },
	handle{ other.handle }
{
	other.handle = nullptr;
}

nano::block_processor::context::~context ()
{
	if (handle != nullptr)
	{
		rsnano::rsn_block_processor_context_destroy (handle);
	}
}

std::shared_ptr<nano::block> nano::block_processor::context::get_block () const
{
	return nano::block_handle_to_block (rsnano::rsn_block_processor_context_block (handle));
}

auto nano::block_processor::context::get_future () -> std::future<result_t>
{
	auto promise = static_cast<std::promise<result_t> *> (rsnano::rsn_block_processor_context_promise (handle));
	return promise->get_future ();
}

void nano::block_processor::context::set_result (result_t const & result)
{
	auto promise = static_cast<std::promise<result_t> *> (rsnano::rsn_block_processor_context_promise (handle));
	promise->set_value (result);
}

/*
 * block_processor
 */

nano::block_processor::block_processor (nano::node & node_a) :
	config (*node_a.config),
	network_params (node_a.network_params),
	flags (node_a.flags),
	stats{ *node_a.stats },
	logger{ *node_a.logger }
{
	auto config_dto{ config.to_dto () };
	handle = rsnano::rsn_block_processor_create (
	this,
	&config_dto,
	node_a.flags.handle,
	node_a.ledger.handle,
	node_a.unchecked.handle,
	node_a.stats->handle,
	&node_a.config->network_params.work.dto);

	batch_processed.add ([this] (auto const & items) {
		// For every batch item: notify the 'processed' observer.
		for (auto const & [result, context] : items)
		{
			block_processed.notify (result, context);
		}
	});
}

nano::block_processor::~block_processor ()
{
	rsnano::rsn_block_processor_destroy (handle);
	// Thread must be stopped before destruction
	debug_assert (!thread.joinable ());
}

rsnano::BlockProcessorHandle const * nano::block_processor::get_handle () const
{
	return handle;
}

void nano::block_processor::start ()
{
	debug_assert (!thread.joinable ());

	thread = std::thread ([this] () {
		nano::thread_role::set (nano::thread_role::name::block_processing);
		run ();
	});
}

void nano::block_processor::stop ()
{
	{
		nano::block_processor_lock lock{ *this };
		stopped = true;
	}
	rsnano::rsn_block_processor_notify_all (handle);
	rsnano::rsn_block_processor_stop (handle);
	if (thread.joinable ())
	{
		thread.join ();
	}
}

std::size_t nano::block_processor::size () const
{
	return rsnano::rsn_block_processor_total_queue_len (handle);
}

std::size_t nano::block_processor::size (nano::block_source source) const
{
	return rsnano::rsn_block_processor_queue_len (handle, static_cast<uint8_t> (source));
}

bool nano::block_processor::full () const
{
	return rsnano::rsn_block_processor_full (handle);
}

bool nano::block_processor::half_full () const
{
	return rsnano::rsn_block_processor_half_full (handle);
}

void nano::block_processor::process_active (std::shared_ptr<nano::block> const & incoming)
{
	add (incoming);
}

bool nano::block_processor::add (std::shared_ptr<nano::block> const & block, block_source const source, std::shared_ptr<nano::transport::channel> const & channel)
{
	auto channel_handle = channel ? channel->handle : nullptr;
	return rsnano::rsn_block_processor_add (handle, block->get_handle (), static_cast<uint8_t> (source), channel_handle);
}

std::optional<nano::block_status> nano::block_processor::add_blocking (std::shared_ptr<nano::block> const & block, block_source const source)
{
	stats.inc (nano::stat::type::blockprocessor, nano::stat::detail::process_blocking);
	logger.debug (nano::log::type::blockprocessor, "Processing block (blocking): {} (source: {})", block->hash ().to_string (), to_string (source));

	context ctx{ block, source };
	auto future = ctx.get_future ();
	rsnano::rsn_block_processor_add_impl (handle, ctx.handle, nullptr);

	try
	{
		auto status = future.wait_for (config.block_process_timeout);
		debug_assert (status != std::future_status::deferred);
		if (status == std::future_status::ready)
		{
			return future.get ();
		}
	}
	catch (std::future_error const &)
	{
		stats.inc (nano::stat::type::blockprocessor, nano::stat::detail::process_blocking_timeout);
		logger.error (nano::log::type::blockprocessor, "Timeout processing block: {}", block->hash ().to_string ());
	}
	return std::nullopt;
}

void nano::block_processor::force (std::shared_ptr<nano::block> const & block_a)
{
	stats.inc (nano::stat::type::blockprocessor, nano::stat::detail::force);
	logger.debug (nano::log::type::blockprocessor, "Forcing block: {}", block_a->hash ().to_string ());

	context ctx{ block_a, block_source::forced };
	rsnano::rsn_block_processor_add_impl (handle, ctx.handle, nullptr);
}

void nano::block_processor::run ()
{
	nano::block_processor_lock lock{ *this };
	while (!stopped)
	{
		if (!lock.queue_empty ())
		{
			lock.unlock ();

			auto processed = process_batch (lock);

			// Set results for futures when not holding the lock
			for (auto & [result, context] : processed)
			{
				context.set_result (result);
			}

			batch_processed.notify (processed);

			lock.lock (handle);
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

auto nano::block_processor::process_batch (nano::block_processor_lock & lock_a) -> std::deque<processed_t>
{
	auto result_handle = rsnano::rsn_block_processor_process_batch (handle);
	std::deque<processed_t> result;
	auto size = rsnano::rsn_process_batch_result_size (result_handle);
	for (auto i = 0; i < size; ++i)
	{
		uint8_t result_code = 0;
		auto ctx_handle = rsnano::rsn_process_batch_result_pop (result_handle, &result_code);
		auto status = static_cast<nano::block_status> (result_code);
		result.emplace_back (status, nano::block_processor::context{ ctx_handle });
	}
	rsnano::rsn_process_batch_result_destroy (result_handle);
	return result;
}

void nano::block_processor::add_rolled_back_observer (std::function<void (std::shared_ptr<nano::block> const &)> observer)
{
	auto context = new std::function<void (std::shared_ptr<nano::block> const &)> (observer);
	rsnano::rsn_block_processor_add_rolled_back_observer (handle, context, block_rolled_back_delete, block_rolled_back_wrapper);
}

void nano::block_processor::notify_block_rolled_back (std::shared_ptr<nano::block> const & block)
{
	rsnano::rsn_block_processor_notify_block_rolled_back (handle, block->get_handle ());
}

std::unique_ptr<nano::container_info_component> nano::block_processor::collect_container_info (std::string const & name)
{
	auto info_handle = rsnano::rsn_block_processor_collect_container_info (handle, name.c_str ());
	return std::make_unique<nano::container_info_composite> (info_handle);
}

std::string_view nano::to_string (nano::block_source source)
{
	return magic_enum::enum_name (source);
}

/*
 * block_processor_config
 */

nano::block_processor_config::block_processor_config (rsnano::BlockProcessorConfigDto const & dto)
{
	max_peer_queue = dto.max_peer_queue;
	max_system_queue = dto.max_system_queue;
	priority_live = dto.priority_live;
	priority_bootstrap = dto.priority_bootstrap;
	priority_local = dto.priority_local;
}

rsnano::BlockProcessorConfigDto nano::block_processor_config::to_dto () const
{
	rsnano::BlockProcessorConfigDto dto;
	dto.max_peer_queue = max_peer_queue;
	dto.max_system_queue = max_system_queue;
	dto.priority_live = priority_live;
	dto.priority_bootstrap = priority_bootstrap;
	dto.priority_local = priority_local;
	return dto;
}

nano::error nano::block_processor_config::deserialize (nano::tomlconfig & toml)
{
	toml.get ("max_peer_queue", max_peer_queue);
	toml.get ("max_system_queue", max_system_queue);
	toml.get ("priority_live", priority_live);
	toml.get ("priority_bootstrap", priority_bootstrap);
	toml.get ("priority_local", priority_local);

	return toml.get_error ();
}
