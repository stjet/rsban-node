#pragma once

#include "nano/lib/rsnano.hpp"
#include "nano/node/transport/traffic_type.hpp"

#include <nano/node/bootstrap/bootstrap_bulk_pull.hpp>
#include <nano/node/common.hpp>
#include <nano/node/transport/socket.hpp>

namespace nano
{
class node;
namespace transport
{
	class channel_tcp;
}

class bootstrap_attempt;
class bootstrap_connections;
class pull_info;
class bootstrap_initiator;

/**
 * Owns the client side of the bootstrap connection.
 */
class bootstrap_client final : public std::enable_shared_from_this<bootstrap_client>
{
public:
	bootstrap_client (rsnano::BootstrapClientHandle * handle_a);
	~bootstrap_client ();
	void stop (bool force);
	double sample_block_rate ();
	double elapsed_seconds () const;
	void set_start_time ();
	void async_read (std::size_t size_a, std::function<void (boost::system::error_code const &, std::size_t)> callback_a);
	void close_socket ();
	void set_timeout (std::chrono::seconds timeout_a);
	uint8_t * get_receive_buffer ();
	nano::tcp_endpoint remote_endpoint () const;
	std::string channel_string () const;
	void send (nano::message & message_a, std::function<void (boost::system::error_code const &, std::size_t)> const & callback_a = nullptr, nano::transport::buffer_drop_policy drop_policy_a = nano::transport::buffer_drop_policy::limiter, nano::transport::traffic_type traffic_type = nano::transport::traffic_type::generic);
	void send_buffer (nano::shared_const_buffer const & buffer_a, std::function<void (boost::system::error_code const &, std::size_t)> const & callback_a = nullptr, nano::transport::buffer_drop_policy policy_a = nano::transport::buffer_drop_policy::limiter, nano::transport::traffic_type traffic_type = nano::transport::traffic_type::generic);
	nano::tcp_endpoint get_tcp_endpoint () const;
	std::shared_ptr<nano::transport::socket> get_socket () const;
	uint64_t get_block_count () const;
	uint64_t inc_block_count (); // returns the previous block count
	double get_block_rate () const;
	bool get_pending_stop () const;
	bool get_hard_stop () const;
	rsnano::BootstrapClientHandle * handle;

private:
	std::vector<uint8_t> buffer; // only used for returning a uint8_t*
};

/**
 * Container for bootstrap_client objects. Owned by bootstrap_initiator which pools open connections and makes them available
 * for use by different bootstrap sessions.
 */
class bootstrap_connections final : public std::enable_shared_from_this<bootstrap_connections>
{
public:
	bootstrap_connections (rsnano::BootstrapConnectionsHandle * handle);
	bootstrap_connections (nano::node & node_a, nano::bootstrap_initiator & initiator);
	bootstrap_connections (bootstrap_connections const &) = delete;
	~bootstrap_connections ();
	void add_connection (nano::endpoint const & endpoint_a);
	unsigned target_connections (std::size_t pulls_remaining, std::size_t attempts_count) const;
	void clear_pulls (uint64_t);
	void run ();
	void stop ();
	void bootstrap_status (boost::property_tree::ptree & tree, std::size_t attempts_count);
	unsigned get_connections_count () const;

	rsnano::BootstrapConnectionsHandle * handle{ nullptr };
};
}
