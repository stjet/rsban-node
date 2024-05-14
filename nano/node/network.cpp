#include "nano/lib/rsnano.hpp"

#include <nano/crypto_lib/random_pool_shuffle.hpp>
#include <nano/lib/blocks.hpp>
#include <nano/lib/rsnanoutils.hpp>
#include <nano/lib/threading.hpp>
#include <nano/node/bootstrap_ascending/service.hpp>
#include <nano/node/network.hpp>
#include <nano/node/node.hpp>
#include <nano/node/telemetry.hpp>

#include <boost/format.hpp>

using namespace std::chrono_literals;

/*
 * network
 */

nano::network::network (nano::node & node, uint16_t port) :
	node{ node },
	id{ nano::network_constants::active_network () },
	syn_cookies{ std::make_shared<nano::syn_cookies> (node.network_params.network.max_peers_per_ip) },
	resolver{ node.io_ctx },
	port{ port }
{
}

nano::network::~network ()
{
}

void nano::network::create_tcp_channels ()
{
	tcp_channels = std::move (std::make_shared<nano::transport::tcp_channels> (node, port));
}

void nano::network::send_keepalive (std::shared_ptr<nano::transport::channel> const & channel_a)
{
	nano::keepalive message{ node.network_params.network };
	std::array<nano::endpoint, 8> peers;
	tcp_channels->random_fill (peers);
	message.set_peers (peers);
	channel_a->send (message);
}

void nano::network::send_keepalive_self (std::shared_ptr<nano::transport::channel> const & channel_a)
{
	nano::keepalive message{ node.network_params.network };
	auto peers{ message.get_peers () };
	fill_keepalive_self (peers);
	message.set_peers (peers);
	channel_a->send (message);
}

void nano::network::flood_message (nano::message & message_a, nano::transport::buffer_drop_policy const drop_policy_a, float const scale_a)
{
	for (auto & i : tcp_channels->random_fanout (scale_a))
	{
		i->send (message_a, nullptr, drop_policy_a);
	}
}

void nano::network::flood_keepalive (float const scale_a)
{
	nano::keepalive message{ node.network_params.network };
	auto peers{ message.get_peers () };
	tcp_channels->random_fill (peers);
	message.set_peers (peers);
	flood_message (message, nano::transport::buffer_drop_policy::limiter, scale_a);
}

void nano::network::flood_keepalive_self (float const scale_a)
{
	nano::keepalive message{ node.network_params.network };
	auto peers{ message.get_peers () };
	fill_keepalive_self (peers);
	message.set_peers (peers);
	flood_message (message, nano::transport::buffer_drop_policy::limiter, scale_a);
}

void nano::network::flood_block (std::shared_ptr<nano::block> const & block_a, nano::transport::buffer_drop_policy const drop_policy_a)
{
	nano::publish message (node.network_params.network, block_a);
	flood_message (message, drop_policy_a);
}

void nano::network::flood_block_many (std::deque<std::shared_ptr<nano::block>> blocks_a, std::function<void ()> callback_a, unsigned delay_a)
{
	if (!blocks_a.empty ())
	{
		auto block_l (blocks_a.front ());
		blocks_a.pop_front ();
		flood_block (block_l);
		if (!blocks_a.empty ())
		{
			std::weak_ptr<nano::node> node_w (node.shared ());
			node.workers->add_timed_task (std::chrono::steady_clock::now () + std::chrono::milliseconds (delay_a + std::rand () % delay_a), [node_w, blocks (std::move (blocks_a)), callback_a, delay_a] () {
				if (auto node_l = node_w.lock ())
				{
					node_l->network->flood_block_many (std::move (blocks), callback_a, delay_a);
				}
			});
		}
		else if (callback_a)
		{
			callback_a ();
		}
	}
}

void nano::network::inbound (const nano::message & message, const std::shared_ptr<nano::transport::channel> & channel)
{
	node.live_message_processor.process (message, channel);
}

// Send keepalives to all the peers we've been notified of
void nano::network::merge_peers (std::array<nano::endpoint, 8> const & peers_a)
{
	for (auto i (peers_a.begin ()), j (peers_a.end ()); i != j; ++i)
	{
		merge_peer (*i);
	}
}

void nano::network::merge_peer (nano::endpoint const & peer_a)
{
	// ported in tcp_channels!
	if (track_reachout (peer_a))
	{
		node.stats->inc (nano::stat::type::network, nano::stat::detail::merge_peer);
		std::weak_ptr<nano::node> node_w (node.shared ());
		node.network->tcp_channels->start_tcp (peer_a);
	}
}

bool nano::network::track_reachout (nano::endpoint const & endpoint_a)
{
	// Don't contact invalid IPs
	if (tcp_channels->not_a_peer (endpoint_a, node.config->allow_local_peers))
	{
		return false;
	}
	return tcp_channels->track_reachout (endpoint_a);
}

std::vector<std::shared_ptr<nano::transport::channel>> nano::network::random_channels (std::size_t count_a, uint8_t min_version_a, bool include_temporary_channels_a) const
{
	return tcp_channels->random_channels (count_a, min_version_a, include_temporary_channels_a);
}

void nano::network::fill_keepalive_self (std::array<nano::endpoint, 8> & target_a) const
{
	tcp_channels->random_fill (target_a);
	// We will clobber values in index 0 and 1 and if there are only 2 nodes in the system, these are the only positions occupied
	// Move these items to index 2 and 3 so they propagate
	target_a[2] = target_a[0];
	target_a[3] = target_a[1];
	// Replace part of message with node external address or listening port
	target_a[1] = nano::endpoint (boost::asio::ip::address_v6{}, 0); // For node v19 (response channels)
	if (node.config->external_address != boost::asio::ip::address_v6{}.to_string () && node.config->external_port != 0)
	{
		target_a[0] = nano::endpoint (boost::asio::ip::make_address_v6 (node.config->external_address), node.config->external_port);
	}
	else
	{
		auto external_address (node.port_mapping.external_address ());
		if (external_address.address () != boost::asio::ip::address_v4::any ())
		{
			target_a[0] = nano::endpoint (boost::asio::ip::address_v6{}, port);
			boost::system::error_code ec;
			auto external_v6 = boost::asio::ip::make_address_v6 (external_address.address ().to_string (), ec);
			target_a[1] = nano::endpoint (external_v6, external_address.port ());
		}
		else
		{
			target_a[0] = nano::endpoint (boost::asio::ip::address_v6{}, port);
		}
	}
}

nano::tcp_endpoint nano::network::bootstrap_peer ()
{
	return tcp_channels->bootstrap_peer ();
}

std::shared_ptr<nano::transport::channel> nano::network::find_node_id (nano::account const & node_id_a)
{
	return tcp_channels->find_node_id (node_id_a);
}

nano::endpoint nano::network::endpoint () const
{
	return nano::endpoint (boost::asio::ip::address_v6::loopback (), port);
}

void nano::network::cleanup (std::chrono::system_clock::time_point const & cutoff_a)
{
	tcp_channels->purge (cutoff_a);
}

std::size_t nano::network::size () const
{
	return tcp_channels->size ();
}

bool nano::network::empty () const
{
	return size () == 0;
}

void nano::network::erase (nano::transport::channel const & channel_a)
{
	auto const channel_type = channel_a.get_type ();
	if (channel_type == nano::transport::transport_type::tcp)
	{
		tcp_channels->erase (channel_a.get_tcp_remote_endpoint ());
	}
}

/*
 * syn_cookies
 */

nano::syn_cookies::syn_cookies (std::size_t max_cookies_per_ip_a) :
	handle{ rsnano::rsn_syn_cookies_create (max_cookies_per_ip_a) }
{
}

nano::syn_cookies::~syn_cookies ()
{
	rsnano::rsn_syn_cookies_destroy (handle);
}

std::optional<nano::uint256_union> nano::syn_cookies::assign (nano::endpoint const & endpoint_a)
{
	auto endpoint_dto{ rsnano::udp_endpoint_to_dto (endpoint_a) };
	nano::uint256_union cookie;
	if (rsnano::rsn_syn_cookies_assign (handle, &endpoint_dto, cookie.bytes.data ()))
	{
		return cookie;
	}

	return std::nullopt;
}

bool nano::syn_cookies::validate (nano::endpoint const & endpoint_a, nano::account const & node_id, nano::signature const & sig)
{
	auto endpoint_dto{ rsnano::udp_endpoint_to_dto (endpoint_a) };
	bool ok = rsnano::rsn_syn_cookies_validate (handle, &endpoint_dto, node_id.bytes.data (), sig.bytes.data ());
	return !ok;
}

void nano::syn_cookies::purge (std::chrono::seconds const & cutoff_a)
{
	rsnano::rsn_syn_cookies_purge (handle, cutoff_a.count ());
}

std::optional<nano::uint256_union> nano::syn_cookies::cookie (const nano::endpoint & endpoint_a)
{
	auto endpoint_dto{ rsnano::udp_endpoint_to_dto (endpoint_a) };
	nano::uint256_union cookie;
	if (rsnano::rsn_syn_cookies_cookie (handle, &endpoint_dto, cookie.bytes.data ()))
	{
		return cookie;
	}
	return std::nullopt;
}

std::size_t nano::syn_cookies::cookies_size ()
{
	return rsnano::rsn_syn_cookies_cookies_count (handle);
}

std::unique_ptr<nano::container_info_component> nano::collect_container_info (network & network, std::string const & name)
{
	auto composite = std::make_unique<container_info_composite> (name);
	composite->add_component (network.tcp_channels->collect_container_info ("tcp_channels"));
	composite->add_component (network.syn_cookies->collect_container_info ("syn_cookies"));
	composite->add_component (network.tcp_channels->excluded_peers ().collect_container_info ("excluded_peers"));
	return composite;
}

std::unique_ptr<nano::container_info_component> nano::syn_cookies::collect_container_info (std::string const & name)
{
	std::size_t syn_cookies_count = rsnano::rsn_syn_cookies_cookies_count (handle);
	std::size_t syn_cookies_per_ip_count = rsnano::rsn_syn_cookies_cookies_per_ip_count (handle);
	auto composite = std::make_unique<container_info_composite> (name);
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "syn_cookies", syn_cookies_count, rsnano::rsn_syn_cookies_cookie_info_size () }));
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "syn_cookies_per_ip", syn_cookies_per_ip_count, rsnano::rsn_syn_cookies_cookies_per_ip_size () }));
	return composite;
}

std::string nano::network::to_string (nano::networks network)
{
	rsnano::StringDto result;
	rsnano::rsn_network_to_string (static_cast<uint16_t> (network), &result);
	return rsnano::convert_dto_to_string (result);
}

void nano::network::on_new_channel (std::function<void (std::shared_ptr<nano::transport::channel>)> observer_a)
{
	tcp_channels->on_new_channel (observer_a);
}

void nano::network::clear_from_publish_filter (nano::uint128_t const & digest_a)
{
	tcp_channels->publish_filter->clear (digest_a);
}

uint16_t nano::network::get_port ()
{
	return port;
}

void nano::network::set_port (uint16_t port_a)
{
	port = port_a;
	tcp_channels->set_port (port_a);
}

nano::live_message_processor::live_message_processor (nano::node & node)
{
	auto config_dto{ node.config->to_dto () };
	handle = rsnano::rsn_live_message_processor_create (node.stats->handle, node.network->tcp_channels->handle,
	node.block_processor.handle, &config_dto, node.flags.handle,
	node.wallets.rust_handle, node.aggregator.handle, node.vote_processor_queue.handle,
	node.telemetry->handle, node.bootstrap_server.handle, node.ascendboot.handle);
}

nano::live_message_processor::~live_message_processor ()
{
	rsnano::rsn_live_message_processor_destroy (handle);
}

void nano::live_message_processor::process (const nano::message & message, const std::shared_ptr<nano::transport::channel> & channel)
{
	rsnano::rsn_live_message_processor_process (handle, message.handle, channel->handle);
}

nano::network_threads::network_threads (nano::node & node)
{
	auto config_dto{ node.config->to_dto () };
	auto params_dto{ node.network_params.to_dto () };
	handle = rsnano::rsn_network_threads_create (
	node.network->tcp_channels->handle,
	&config_dto,
	node.flags.handle,
	&params_dto,
	node.stats->handle,
	node.network->syn_cookies->handle);
}

nano::network_threads::~network_threads ()
{
	rsnano::rsn_network_threads_destroy (handle);
}

void nano::network_threads::start ()
{
	rsnano::rsn_network_threads_start (handle);
}

void nano::network_threads::stop ()
{
	rsnano::rsn_network_threads_stop (handle);
}
