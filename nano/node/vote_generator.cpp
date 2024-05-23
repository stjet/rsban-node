#include <nano/lib/rsnano.hpp>
#include <nano/lib/rsnanoutils.hpp>
#include <nano/node/local_vote_history.hpp>
#include <nano/node/messages.hpp>
#include <nano/node/network.hpp>
#include <nano/node/node.hpp>
#include <nano/node/transport/inproc.hpp>
#include <nano/node/vote_generator.hpp>
#include <nano/node/vote_processor.hpp>

nano::vote_generator::vote_generator (nano::node & node_a, nano::node_config const & config_a, nano::ledger & ledger_a, nano::wallets & wallets_a, nano::vote_processor & vote_processor_a, nano::vote_processor_queue & vote_processor_queue_a, nano::local_vote_history & history_a, nano::network & network_a, nano::stats & stats_a, nano::representative_register & representative_register_a, bool is_final_a)
{
	auto network_constants_dto{ config_a.network_params.network.to_dto () };
	auto context = new std::function<void (nano::message const &, std::shared_ptr<nano::transport::channel> const &)> ([&network_a] (nano::message const & msg, std::shared_ptr<nano::transport::channel> const & channel) { network_a.inbound (msg, channel); });
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

nano::vote_generator::vote_generator (rsnano::VoteGeneratorHandle * handle) :
	handle{ handle }
{
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
