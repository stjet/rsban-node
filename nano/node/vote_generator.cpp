#include <nano/lib/rsnano.hpp>
#include <nano/lib/rsnanoutils.hpp>
#include <nano/node/local_vote_history.hpp>
#include <nano/node/messages.hpp>
#include <nano/node/network.hpp>
#include <nano/node/node.hpp>
#include <nano/node/transport/inproc.hpp>
#include <nano/node/vote_generator.hpp>
#include <nano/node/vote_processor.hpp>

nano::vote_generator::vote_generator (rsnano::VoteGeneratorHandle * handle) :
	handle{ handle }
{
}

nano::vote_generator::~vote_generator ()
{
	rsnano::rsn_vote_generator_destroy (handle);
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
