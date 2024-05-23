#pragma once

#include "nano/lib/rsnano.hpp"

#include <nano/node/common.hpp>
#include <nano/node/peer_exclusion.hpp>
#include <nano/node/transport/tcp.hpp>
#include <nano/secure/network_filter.hpp>

#include <boost/thread/thread.hpp>

#include <chrono>
#include <deque>
#include <memory>

namespace nano
{
class node;

/**
 * Node ID cookies for node ID handshakes
 */
class syn_cookies final
{
public:
	explicit syn_cookies (rsnano::SynCookiesHandle * handle);
	syn_cookies (nano::syn_cookies const &) = delete;
	~syn_cookies ();
	void purge (std::chrono::seconds const &);

	// Returns boost::none if the IP is rate capped on syn cookie requests,
	// or if the endpoint already has a syn cookie query
	std::optional<nano::uint256_union> assign (nano::endpoint const &);

	std::size_t cookies_size ();
	rsnano::SynCookiesHandle * handle;
};

class network final : public std::enable_shared_from_this<network>
{
public:
	network (nano::node &, uint16_t port, rsnano::SynCookiesHandle * syn_cookies_handle, rsnano::TcpChannelsHandle * channels_handle, rsnano::TcpMessageManagerHandle * mgr_handle, rsnano::NetworkFilterHandle * filter_handle);
	~network ();

	void flood_message (nano::message &, nano::transport::buffer_drop_policy const = nano::transport::buffer_drop_policy::limiter, float const = 1.0f);
	// Flood block to a random selection of peers
	void flood_block (std::shared_ptr<nano::block> const &, nano::transport::buffer_drop_policy const = nano::transport::buffer_drop_policy::limiter);
	void flood_block_many (std::deque<std::shared_ptr<nano::block>>, std::function<void ()> = nullptr, unsigned = broadcast_interval_ms);
	void merge_peers (std::array<nano::endpoint, 8> const &);
	void merge_peer (nano::endpoint const &);
	void send_keepalive (std::shared_ptr<nano::transport::channel> const &);
	std::shared_ptr<nano::transport::channel> find_node_id (nano::account const &);
	// Should we reach out to this endpoint with a keepalive message? If yes, register a new reachout attempt
	bool track_reachout (nano::endpoint const &);
	// Note: The minimum protocol version is used after the random selection, so number of peers can be less than expected.
	std::vector<std::shared_ptr<nano::transport::channel>> random_channels (std::size_t count, uint8_t min_version = 0, bool include_temporary_channels = false) const;
	nano::endpoint endpoint () const;
	void cleanup (std::chrono::system_clock::time_point const & cutoff);
	std::size_t size () const;
	bool empty () const;
	/** Disconnects and adds peer to exclusion list */
	void inbound (nano::message const &, std::shared_ptr<nano::transport::channel> const &);

	static std::string to_string (nano::networks);

private: // Dependencies
	nano::node & node;

public:
	std::shared_ptr<nano::syn_cookies> syn_cookies;
	std::shared_ptr<nano::transport::tcp_channels> tcp_channels;
	std::atomic<uint16_t> port{ 0 };

public:
	static unsigned const broadcast_interval_ms = 10;

	static std::size_t const confirm_req_hashes_max = 7;
	static std::size_t const confirm_ack_hashes_max = 12;
};

class live_message_processor
{
public:
	live_message_processor (rsnano::LiveMessageProcessorHandle * handle);
	live_message_processor (live_message_processor const &) = delete;
	~live_message_processor ();

	void process (const nano::message & message, const std::shared_ptr<nano::transport::channel> & channel);

	rsnano::LiveMessageProcessorHandle * handle;
};

class network_threads
{
public:
	network_threads (rsnano::NetworkThreadsHandle * handle);
	network_threads (network_threads const &) = delete;
	~network_threads ();

	void start ();
	void stop ();

	rsnano::NetworkThreadsHandle * handle;
};
}
