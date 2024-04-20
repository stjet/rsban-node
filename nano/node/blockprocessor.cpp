#include "nano/lib/blocks.hpp"
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

void block_processed_wrapper (void * context, rsnano::BlockProcessedInfoDto * dto)
{
	auto callback = static_cast<std::function<void (nano::block_status, std::shared_ptr<nano::block> const &, nano::block_source)> *> (context);
	auto block{ nano::block_handle_to_block (dto->block) };
	(*callback) (static_cast<nano::block_status> (dto->status), block, static_cast<nano::block_source> (dto->source));
}

void block_processed_delete (void * context)
{
	auto callback = static_cast<std::function<void (nano::block_status, std::shared_ptr<nano::block> const &, nano::block_source)> *> (context);
	delete callback;
}

void batch_processed_wrapper (void * context, rsnano::BlockProcessedInfoDto const * dto, std::size_t len)
{
	auto callback = static_cast<std::function<void (nano::block_processor::processed_batch_t const &)> *> (context);
	std::vector<std::tuple<nano::block_status, std::shared_ptr<nano::block>, nano::block_source>> blocks{};
	for (auto i = 0; i < len; ++i)
	{
		auto block{ nano::block_handle_to_block (dto->block) };
		blocks.emplace_back (static_cast<nano::block_status> (dto->status), block, static_cast<nano::block_source> (dto->source));
		++dto;
	}

	(*callback) (blocks);
}

void batch_processed_delete (void * context)
{
	auto callback = static_cast<std::function<void (nano::block_processor::processed_batch_t const &)> *> (context);
	delete callback;
}
}

/*
 * block_processor
 */

nano::block_processor::block_processor (nano::node & node_a)
{
	auto config_dto{ node_a.config->to_dto () };
	handle = rsnano::rsn_block_processor_create (
	&config_dto,
	node_a.flags.handle,
	node_a.ledger.handle,
	node_a.unchecked.handle,
	node_a.stats->handle,
	&node_a.config->network_params.work.dto);
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
	rsnano::rsn_block_processor_start (handle);
}

void nano::block_processor::stop ()
{
	rsnano::rsn_block_processor_stop (handle);
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
	std::uint8_t status;
	if (rsnano::rsn_block_processor_add_blocking (handle, block->get_handle (), static_cast<uint8_t> (source), &status))
	{
		return static_cast<nano::block_status> (status);
	}
	else
	{
		return std::nullopt;
	}
}

void nano::block_processor::force (std::shared_ptr<nano::block> const & block_a)
{
	rsnano::rsn_block_processor_force (handle, block_a->get_handle ());
}

void nano::block_processor::set_blocks_rolled_back_callback (std::function<void (std::vector<std::shared_ptr<nano::block>> const &, std::shared_ptr<nano::block> const &)> callback)
{
	rsnano::rsn_block_processor_set_blocks_rolled_back_callback (
	handle,
	blocks_rolled_back_wrapper,
	new std::function<void (std::vector<std::shared_ptr<nano::block>> const &, std::shared_ptr<nano::block> const &)> (callback),
	blocks_rolled_back_delete);
}

void nano::block_processor::add_batch_processed_observer (std::function<void (nano::block_processor::processed_batch_t const &)> observer)
{
	auto context = new std::function<void (nano::block_processor::processed_batch_t const &)> (observer);
	rsnano::rsn_block_processor_add_batch_processed_observer (handle, context, batch_processed_delete, batch_processed_wrapper);
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
