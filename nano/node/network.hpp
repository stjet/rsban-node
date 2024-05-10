#pragma once

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
	explicit syn_cookies (std::size_t max_peers_per_ip);
	syn_cookies (nano::syn_cookies const &) = delete;
	~syn_cookies ();
	void purge (std::chrono::seconds const &);

	// Returns boost::none if the IP is rate capped on syn cookie requests,
	// or if the endpoint already has a syn cookie query
	std::optional<nano::uint256_union> assign (nano::endpoint const &);
	// Returns false if valid, true if invalid (true on error convention)
	// Also removes the syn cookie from the store if valid
	bool validate (nano::endpoint const &, nano::account const &, nano::signature const &);
	/** Get cookie associated with endpoint and erases that cookie from this container */
	std::optional<nano::uint256_union> cookie (nano::endpoint const &);

	std::unique_ptr<container_info_component> collect_container_info (std::string const &);
	std::size_t cookies_size ();
	rsnano::SynCookiesHandle * handle;
};

class network final : public std::enable_shared_from_this<network>
{
public:
	network (nano::node &, uint16_t port);
	~network ();

	void create_tcp_channels ();
	void start ();
	void stop ();
	void flood_message (nano::message &, nano::transport::buffer_drop_policy const = nano::transport::buffer_drop_policy::limiter, float const = 1.0f);
	void flood_keepalive (float const scale_a = 1.0f);
	void flood_keepalive_self (float const scale_a = 0.5f);
	// Flood block to a random selection of peers
	void flood_block (std::shared_ptr<nano::block> const &, nano::transport::buffer_drop_policy const = nano::transport::buffer_drop_policy::limiter);
	void flood_block_many (std::deque<std::shared_ptr<nano::block>>, std::function<void ()> = nullptr, unsigned = broadcast_interval_ms);
	void merge_peers (std::array<nano::endpoint, 8> const &);
	void merge_peer (nano::endpoint const &);
	void send_keepalive (std::shared_ptr<nano::transport::channel> const &);
	void send_keepalive_self (std::shared_ptr<nano::transport::channel> const &);
	std::shared_ptr<nano::transport::channel> find_node_id (nano::account const &);
	// Should we reach out to this endpoint with a keepalive message? If yes, register a new reachout attempt
	bool track_reachout (nano::endpoint const &);
	void fill_keepalive_self (std::array<nano::endpoint, 8> &) const;
	// Note: The minimum protocol version is used after the random selection, so number of peers can be less than expected.
	std::vector<std::shared_ptr<nano::transport::channel>> random_channels (std::size_t count, uint8_t min_version = 0, bool include_temporary_channels = false) const;
	// Get the next peer for attempting a tcp bootstrap connection
	nano::tcp_endpoint bootstrap_peer ();
	nano::endpoint endpoint () const;
	void cleanup (std::chrono::system_clock::time_point const & cutoff);
	std::size_t size () const;
	bool empty () const;
	void erase (nano::transport::channel const &);
	/** Disconnects and adds peer to exclusion list */
	void inbound (nano::message const &, std::shared_ptr<nano::transport::channel> const &);

	static std::string to_string (nano::networks);
	void on_new_channel (std::function<void (std::shared_ptr<nano::transport::channel>)> observer_a);
	void clear_from_publish_filter (nano::uint128_t const & digest_a);
	uint16_t get_port ();
	void set_port (uint16_t port_a);

private:
	void run_processing ();
	void run_cleanup ();
	void run_keepalive ();
	void run_reachout ();
	void process_message (nano::message const &, std::shared_ptr<nano::transport::channel> const &);

private: // Dependencies
	nano::node & node;

public:
	nano::networks const id;
	std::shared_ptr<nano::syn_cookies> syn_cookies;
	boost::asio::ip::udp::resolver resolver;
	std::shared_ptr<nano::transport::tcp_channels> tcp_channels;
	std::atomic<uint16_t> port{ 0 };

public: // Callbacks
	std::function<void ()> disconnect_observer{ [] () {} };

private:
	std::atomic<bool> stopped{ false };
	mutable nano::mutex mutex;
	nano::condition_variable condition;
	std::vector<boost::thread> processing_threads; // Using boost::thread to enable increased stack size
	std::thread cleanup_thread;
	std::thread keepalive_thread;
	std::thread reachout_thread;

public:
	static unsigned const broadcast_interval_ms = 10;
	static std::size_t const buffer_size = 512;

	static std::size_t const confirm_req_hashes_max = 7;
	static std::size_t const confirm_ack_hashes_max = 12;
};

std::unique_ptr<container_info_component> collect_container_info (network & network, std::string const & name);
}
