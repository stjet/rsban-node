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

nano::bootstrap_server::bootstrap_server (rsnano::BootstrapServerHandle * handle) :
	handle{ handle }
{
}

nano::bootstrap_server::~bootstrap_server ()
{
	rsnano::rsn_bootstrap_server_destroy (handle);
}

namespace
{
void response_callback_wrapper (void * context, rsnano::MessageHandle * msg_handle, rsnano::ChannelHandle * channel_handle)
{
	auto callback = static_cast<std::function<void (nano::asc_pull_ack const &, std::shared_ptr<nano::transport::channel> &)> *> (context);
	nano::asc_pull_ack message{ msg_handle };
	auto channel{ nano::transport::channel_handle_to_channel (channel_handle) };
	(*callback) (message, channel);
}

void delete_context (void * context)
{
	auto callback = static_cast<std::function<void (nano::asc_pull_ack const &, std::shared_ptr<nano::transport::channel> &)> *> (context);
	delete callback;
}
}

void nano::bootstrap_server::set_response_callback (std::function<void (nano::asc_pull_ack const &, std::shared_ptr<nano::transport::channel> &)> callback)
{
	auto context = new std::function<void (nano::asc_pull_ack const &, std::shared_ptr<nano::transport::channel> &)> (callback);
	rsnano::rsn_bootstrap_server_set_callback (handle, response_callback_wrapper, context, delete_context);
}

/*
 * bootstrap_server_config
 */

nano::error nano::bootstrap_server_config::deserialize (nano::tomlconfig & toml)
{
	toml.get ("max_queue", max_queue);
	toml.get ("threads", threads);
	toml.get ("batch_size", batch_size);

	return toml.get_error ();
}

void nano::bootstrap_server_config::load_dto (rsnano::BootstrapServerConfigDto const & dto)
{
	max_queue = dto.max_queue;
	threads = dto.threads;
	batch_size = dto.batch_size;
}

rsnano::BootstrapServerConfigDto nano::bootstrap_server_config::to_dto () const
{
	return { max_queue, threads, batch_size };
}
