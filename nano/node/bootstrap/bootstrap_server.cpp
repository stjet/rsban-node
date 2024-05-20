#include "nano/lib/rsnano.hpp"
#include "nano/node/messages.hpp"
#include "nano/node/transport/tcp.hpp"

#include <nano/lib/blocks.hpp>
#include <nano/lib/utility.hpp>
#include <nano/node/bootstrap/bootstrap_server.hpp>
#include <nano/node/transport/channel.hpp>
#include <nano/node/transport/transport.hpp>
#include <nano/secure/ledger.hpp>
#include <nano/store/account.hpp>
#include <nano/store/block.hpp>
#include <nano/store/component.hpp>
#include <nano/store/confirmation_height.hpp>

// TODO: Make threads configurable
nano::bootstrap_server::bootstrap_server (nano::store::component & store_a, nano::ledger & ledger_a, nano::network_constants const & network_constants_a, nano::stats & stats_a) :
	handle{ rsnano::rsn_bootstrap_server_create (stats_a.handle, ledger_a.handle) }
{
}

nano::bootstrap_server::bootstrap_server (rsnano::BootstrapServerHandle * handle) :
	handle{ handle }
{
}

nano::bootstrap_server::~bootstrap_server ()
{
	rsnano::rsn_bootstrap_server_destroy (handle);
}

void nano::bootstrap_server::start ()
{
	rsnano::rsn_bootstrap_server_start (handle);
}

void nano::bootstrap_server::stop ()
{
	rsnano::rsn_bootstrap_server_stop (handle);
}

bool nano::bootstrap_server::request (nano::asc_pull_req const & message, std::shared_ptr<nano::transport::channel> channel)
{
	return rsnano::rsn_bootstrap_server_request (handle, message.handle, channel->handle);
}

namespace
{
void response_callback_wrapper (void * context, rsnano::MessageHandle * msg_handle, rsnano::ChannelHandle * channel_handle)
{
	auto callback = static_cast<std::function<void (nano::asc_pull_ack &, std::shared_ptr<nano::transport::channel> &)> *> (context);
	nano::asc_pull_ack message{ msg_handle };
	auto channel{ nano::transport::channel_handle_to_channel (channel_handle) };
	(*callback) (message, channel);
}

void delete_context (void * context)
{
	auto callback = static_cast<std::function<void (nano::asc_pull_ack &, std::shared_ptr<nano::transport::channel> &)> *> (context);
	delete callback;
}
}

void nano::bootstrap_server::set_response_callback (std::function<void (nano::asc_pull_ack &, std::shared_ptr<nano::transport::channel> &)> callback)
{
	auto context = new std::function<void (nano::asc_pull_ack &, std::shared_ptr<nano::transport::channel> &)> (callback);
	rsnano::rsn_bootstrap_server_set_callback (handle, response_callback_wrapper, context, delete_context);
}
