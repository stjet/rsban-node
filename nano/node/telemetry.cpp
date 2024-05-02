#include "nano/lib/observer_set.hpp"
#include "nano/lib/rsnano.hpp"
#include "nano/lib/rsnanoutils.hpp"
#include "nano/node/messages.hpp"
#include "nano/node/transport/tcp.hpp"

#include <nano/lib/blocks.hpp>
#include <nano/lib/stats.hpp>
#include <nano/lib/threading.hpp>
#include <nano/node/network.hpp>
#include <nano/node/node.hpp>
#include <nano/node/node_observers.hpp>
#include <nano/node/telemetry.hpp>
#include <nano/node/transport/transport.hpp>
#include <nano/secure/ledger.hpp>

#include <boost/algorithm/string.hpp>

#include <memory>
#include <optional>

using namespace std::chrono_literals;

namespace
{
void notify_wrapper (void * context, rsnano::TelemetryDataHandle * data_handle, rsnano::ChannelHandle * channel_handle)
{
	auto observers = static_cast<std::shared_ptr<nano::node_observers> *> (context);
	nano::telemetry_data data{ data_handle };
	auto channel{ nano::transport::channel_handle_to_channel (channel_handle) };
	(*observers)->telemetry.notify (data, channel);
}

void delete_notify_context (void * context)
{
	auto observers = static_cast<std::shared_ptr<nano::node_observers> *> (context);
	delete observers;
}
}

nano::telemetry::telemetry (const config & config_a, nano::node & node_a, nano::network & network_a, nano::node_observers & observers_a, nano::network_params & network_params_a, nano::stats & stats_a)
{
	auto node_config_dto{ node_a.config->to_dto () };
	auto network_dto{ node_a.network_params.to_dto () };
	auto context = new std::shared_ptr<nano::node_observers> (node_a.observers);
	handle = rsnano::rsn_telemetry_create (
	config_a.enable_ongoing_requests,
	config_a.enable_ongoing_broadcasts,
	&node_config_dto,
	node_a.stats->handle,
	node_a.ledger.handle,
	node_a.unchecked.handle,
	&network_dto,
	node_a.network->tcp_channels->handle,
	node_a.node_id.prv.bytes.data (),
	notify_wrapper,
	context,
	delete_notify_context);
}

nano::telemetry::~telemetry ()
{
	rsnano::rsn_telemetry_destroy (handle);
}

void nano::telemetry::start ()
{
	rsnano::rsn_telemetry_start (handle);
}

void nano::telemetry::stop ()
{
	rsnano::rsn_telemetry_stop (handle);
}

void nano::telemetry::process (const nano::telemetry_ack & telemetry, const std::shared_ptr<nano::transport::channel> & channel)
{
	rsnano::rsn_telemetry_process (handle, telemetry.handle, channel->handle);
}

void nano::telemetry::trigger ()
{
	rsnano::rsn_telemetry_trigger (handle);
}

nano::telemetry_data nano::telemetry::local_telemetry () const
{
	return { rsnano::rsn_telemetry_local_telemetry (handle) };
}

std::size_t nano::telemetry::size () const
{
	return rsnano::rsn_telemetry_len (handle);
}

std::optional<nano::telemetry_data> nano::telemetry::get_telemetry (const nano::endpoint & endpoint) const
{
	auto dto{ rsnano::udp_endpoint_to_dto (endpoint) };
	auto data_handle = rsnano::rsn_telemetry_get_telemetry (handle, &dto);
	if (data_handle != nullptr)
	{
		return { nano::telemetry_data{ data_handle } };
	}
	else
	{
		return std::nullopt;
	}
}

std::unordered_map<nano::endpoint, nano::telemetry_data> nano::telemetry::get_all_telemetries () const
{
	auto map_handle = rsnano::rsn_telemetry_get_all (handle);
	auto size = rsnano::rsn_telemetry_data_map_len (map_handle);
	std::unordered_map<nano::endpoint, nano::telemetry_data> result;
	for (auto i = 0; i < size; ++i)
	{
		rsnano::EndpointDto endpoint_dto;
		auto data_handle = rsnano::rsn_telemetry_data_map_get (map_handle, i, &endpoint_dto);
		auto endpoint = rsnano::dto_to_udp_endpoint (endpoint_dto);
		result.emplace (endpoint, nano::telemetry_data{ data_handle });
	}
	rsnano::rsn_telemetry_data_map_destroy (map_handle);
	return result;
}

std::unique_ptr<nano::container_info_component> nano::telemetry::collect_container_info (const std::string & name)
{
	return std::make_unique<container_info_composite> (rsnano::rsn_telemetry_collect_container_info (handle, name.c_str ()));
}

nano::telemetry_data nano::consolidate_telemetry_data (std::vector<nano::telemetry_data> const & telemetry_datas)
{
	std::vector<rsnano::TelemetryDataHandle *> data_handles;
	data_handles.reserve (telemetry_datas.size ());
	for (auto const & i : telemetry_datas)
	{
		data_handles.push_back (i.handle);
	}
	return { rsnano::rsn_consolidate_telemetry_data (data_handles.data (), data_handles.size ()) };
}
