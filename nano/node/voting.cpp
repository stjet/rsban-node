#include "nano/lib/rsnano.hpp"
#include "nano/lib/rsnanoutils.hpp"
#include "nano/node/repcrawler.hpp"
#include "nano/node/transport/tcp.hpp"

#include <nano/lib/blocks.hpp>
#include <nano/lib/stats.hpp>
#include <nano/lib/threading.hpp>
#include <nano/lib/utility.hpp>
#include <nano/node/network.hpp>
#include <nano/node/node.hpp>
#include <nano/node/nodeconfig.hpp>
#include <nano/node/transport/inproc.hpp>
#include <nano/node/vote_processor.hpp>
#include <nano/node/voting.hpp>
#include <nano/node/wallet.hpp>
#include <nano/secure/ledger.hpp>
#include <nano/store/component.hpp>

#include <chrono>
#include <memory>

nano::vote_spacing::vote_spacing (std::chrono::milliseconds const & delay) :
	handle{ rsnano::rsn_vote_spacing_create (delay.count ()) }
{
}

nano::vote_spacing::~vote_spacing ()
{
	rsnano::rsn_vote_spacing_destroy (handle);
}

bool nano::vote_spacing::votable (nano::root const & root_a, nano::block_hash const & hash_a) const
{
	return rsnano::rsn_vote_spacing_votable (handle, root_a.bytes.data (), hash_a.bytes.data ());
}

void nano::vote_spacing::flag (nano::root const & root_a, nano::block_hash const & hash_a)
{
	rsnano::rsn_vote_spacing_flag (handle, root_a.bytes.data (), hash_a.bytes.data ());
}

std::size_t nano::vote_spacing::size () const
{
	return rsnano::rsn_vote_spacing_len (handle);
}

nano::local_vote_history::local_vote_history (nano::voting_constants const & constants) :
	handle{ rsnano::rsn_local_vote_history_create (constants.max_cache) }
{
}

nano::local_vote_history::~local_vote_history ()
{
	rsnano::rsn_local_vote_history_destroy (handle);
}

void nano::local_vote_history::add (nano::root const & root_a, nano::block_hash const & hash_a, std::shared_ptr<nano::vote> const & vote_a)
{
	rsnano::rsn_local_vote_history_add (handle, root_a.bytes.data (), hash_a.bytes.data (), vote_a->get_handle ());
}

void nano::local_vote_history::erase (nano::root const & root_a)
{
	rsnano::rsn_local_vote_history_erase (handle, root_a.bytes.data ());
}

class LocalVotesResultWrapper
{
public:
	LocalVotesResultWrapper () :
		result{}
	{
	}
	~LocalVotesResultWrapper ()
	{
		rsnano::rsn_local_vote_history_votes_destroy (result.handle);
	}
	rsnano::LocalVotesResult result;
};

std::vector<std::shared_ptr<nano::vote>> nano::local_vote_history::votes (nano::root const & root_a, nano::block_hash const & hash_a, bool const is_final_a) const
{
	LocalVotesResultWrapper result_wrapper;
	rsnano::rsn_local_vote_history_votes (handle, root_a.bytes.data (), hash_a.bytes.data (), is_final_a, &result_wrapper.result);
	std::vector<std::shared_ptr<nano::vote>> votes;
	votes.reserve (result_wrapper.result.count);
	for (auto i (0); i < result_wrapper.result.count; ++i)
	{
		votes.push_back (std::make_shared<nano::vote> (result_wrapper.result.votes[i]));
	}
	return votes;
}

bool nano::local_vote_history::exists (nano::root const & root_a) const
{
	return rsnano::rsn_local_vote_history_exists (handle, root_a.bytes.data ());
}

std::size_t nano::local_vote_history::size () const
{
	return rsnano::rsn_local_vote_history_size (handle);
}

std::unique_ptr<nano::container_info_component> nano::collect_container_info (nano::local_vote_history & history, std::string const & name)
{
	std::size_t sizeof_element;
	std::size_t history_count;
	rsnano::rsn_local_vote_history_container_info (history.handle, &sizeof_element, &history_count);
	auto composite = std::make_unique<container_info_composite> (name);
	/* This does not currently loop over each element inside the cache to get the sizes of the votes inside history*/
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "history", history_count, sizeof_element }));
	return composite;
}

nano::vote_broadcaster::vote_broadcaster (nano::node & node_a, nano::vote_processor_queue & vote_processor_queue_a, nano::network & network_a, nano::representative_register & representative_register_a, nano::network_params const & network_params_a, nano::transport::tcp_channels & tcp_channels_a)
{
	auto network_constants_dto{ network_params_a.network.to_dto () };
	auto context = new std::function<void (nano::message const &, std::shared_ptr<nano::transport::channel> const &)> (network_a.inbound);
	auto endpoint_dto{ rsnano::udp_endpoint_to_dto (network_a.endpoint ()) };
	handle = rsnano::rsn_vote_broadcaster_create (
	representative_register_a.handle,
	tcp_channels_a.handle,
	vote_processor_queue_a.handle,
	&network_constants_dto,
	node_a.stats->handle,
	node_a.async_rt.handle,
	node_a.node_id.pub.bytes.data (),
	&endpoint_dto,
	nano::transport::inbound_wrapper,
	context,
	nano::transport::delete_inbound_context);
}

nano::vote_broadcaster::~vote_broadcaster ()
{
	rsnano::rsn_vote_broadcaster_destroy (handle);
}

void nano::vote_broadcaster::broadcast (std::shared_ptr<nano::vote> const & vote_a) const
{
	rsnano::rsn_vote_broadcaster_broadcast (handle, vote_a->get_handle ());
}

nano::vote_generator::vote_generator (nano::node & node_a, nano::node_config const & config_a, nano::ledger & ledger_a, nano::wallets & wallets_a, nano::vote_processor & vote_processor_a, nano::vote_processor_queue & vote_processor_queue_a, nano::local_vote_history & history_a, nano::network & network_a, nano::stats & stats_a, nano::representative_register & representative_register_a, bool is_final_a)
{
	auto network_constants_dto{ config_a.network_params.network.to_dto () };
	auto context = new std::function<void (nano::message const &, std::shared_ptr<nano::transport::channel> const &)> (network_a.inbound);
	auto endpoint_dto{ rsnano::udp_endpoint_to_dto (network_a.endpoint ()) };

	handle = rsnano::rsn_vote_generator_create (
	ledger_a.handle,
	node_a.wallets.rust_handle,
	history_a.handle,
	is_final_a,
	stats_a.handle,
	representative_register_a.handle,
	network_a.tcp_channels->handle,
	vote_processor_queue_a.handle,
	&network_constants_dto,
	node_a.async_rt.handle,
	node_a.node_id.pub.bytes.data (),
	&endpoint_dto,
	nano::transport::inbound_wrapper,
	context,
	nano::transport::delete_inbound_context,
	config_a.network_params.voting.delay.count (),
	config_a.vote_generator_delay.count (),
	config_a.vote_generator_threshold);
}

nano::vote_generator::~vote_generator ()
{
	rsnano::rsn_vote_generator_destroy (handle);
}

void nano::vote_generator::start ()
{
	rsnano::rsn_vote_generator_start (handle);
}

void nano::vote_generator::stop ()
{
	rsnano::rsn_vote_generator_stop (handle);
}

void nano::vote_generator::add (const root & root, const block_hash & hash)
{
	rsnano::rsn_vote_generator_add (handle, root.bytes.data (), hash.bytes.data ());
}

std::size_t nano::vote_generator::generate (std::vector<std::shared_ptr<nano::block>> const & blocks_a, std::shared_ptr<nano::transport::channel> const & channel_a)
{
	rsnano::block_vec block_vec{ blocks_a };
	return rsnano::rsn_vote_generator_generate (handle, block_vec.handle, channel_a->handle);
}

namespace
{
void call_reply_action (void * context, rsnano::VoteHandle * vote_handle, rsnano::ChannelHandle * channel_handle)
{
	auto action = static_cast<std::function<void (std::shared_ptr<nano::vote> const &, std::shared_ptr<nano::transport::channel> const &)> *> (context);
	auto vote{ std::make_shared<nano::vote> (vote_handle) };
	auto channel{ nano::transport::channel_handle_to_channel (channel_handle) };
	(*action) (vote, channel);
}

void drop_reply_action_context (void * context)
{
	auto action = static_cast<std::function<void (std::shared_ptr<nano::vote> const &, std::shared_ptr<nano::transport::channel> const &)> *> (context);
	delete action;
}
}

void nano::vote_generator::set_reply_action (std::function<void (std::shared_ptr<nano::vote> const &, std::shared_ptr<nano::transport::channel> const &)> action_a)
{
	auto context = new std::function<void (std::shared_ptr<nano::vote> const &, std::shared_ptr<nano::transport::channel> const &)> (action_a);
	rsnano::rsn_vote_generator_set_reply_action (handle, call_reply_action, context, drop_reply_action_context);
}

std::unique_ptr<nano::container_info_component> nano::collect_container_info (nano::vote_generator & vote_generator, std::string const & name)
{
	std::size_t candidates_count = 0;
	std::size_t requests_count = 0;
	// TODO:
	/*{
		nano::lock_guard<nano::mutex> guard{ vote_generator.mutex };
		candidates_count = vote_generator.candidates.size ();
		requests_count = vote_generator.requests.size ();
	}
	auto sizeof_candidate_element = sizeof (decltype (vote_generator.candidates)::value_type);
	auto sizeof_request_element = sizeof (decltype (vote_generator.requests)::value_type);
	*/
	auto composite = std::make_unique<container_info_composite> (name);
	/*
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "candidates", candidates_count, sizeof_candidate_element }));
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "requests", requests_count, sizeof_request_element }));
	composite->add_component (vote_generator.vote_generation_queue.collect_container_info ("vote_generation_queue"));
	*/
	return composite;
}
