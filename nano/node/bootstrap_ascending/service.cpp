#include <nano/lib/blocks.hpp>
#include <nano/lib/rsnano.hpp>
#include <nano/lib/stats_enums.hpp>
#include <nano/node/blockprocessor.hpp>
#include <nano/node/bootstrap_ascending/service.hpp>
#include <nano/node/network.hpp>
#include <nano/node/nodeconfig.hpp>
#include <nano/node/transport/transport.hpp>
#include <nano/secure/common.hpp>
#include <nano/secure/ledger.hpp>
#include <nano/store/account.hpp>
#include <nano/store/component.hpp>

using namespace std::chrono_literals;

/*
 * bootstrap_ascending
 */

nano::bootstrap_ascending::service::service (nano::node_config & config_a, nano::block_processor & block_processor_a, nano::ledger & ledger_a, nano::network & network_a, nano::stats & stat_a)
{
	auto config_dto{ config_a.to_dto () };
	auto constants_dto{ config_a.network_params.network.to_dto () };
	handle = rsnano::rsn_bootstrap_ascending_create (block_processor_a.handle, ledger_a.handle, stat_a.handle,
	network_a.tcp_channels->handle, &config_dto, &constants_dto);

	rsnano::rsn_bootstrap_ascending_initialize (handle);
}

nano::bootstrap_ascending::service::service (rsnano::BootstrapAscendingHandle * handle) :
	handle{ handle }
{
}

nano::bootstrap_ascending::service::~service ()
{
	rsnano::rsn_bootstrap_ascending_destroy (handle);
}

void nano::bootstrap_ascending::service::start ()
{
	rsnano::rsn_bootstrap_ascending_start (handle);
}

void nano::bootstrap_ascending::service::stop ()
{
	rsnano::rsn_bootstrap_ascending_stop (handle);
}

void nano::bootstrap_ascending::service::process (nano::asc_pull_ack const & message, std::shared_ptr<nano::transport::channel> channel)
{
	rsnano::rsn_bootstrap_ascending_process (handle, message.handle, channel->handle);
}

std::unique_ptr<nano::container_info_component> nano::bootstrap_ascending::service::collect_container_info (std::string const & name)
{
	return std::make_unique<container_info_composite> (rsnano::rsn_bootstrap_ascending_collect_container_info (handle, name.c_str ()));
}
