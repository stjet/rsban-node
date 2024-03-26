#pragma once

#include <nano/lib/locks.hpp>
#include <nano/lib/stats.hpp>
#include <nano/node/bandwidth_limiter.hpp>
#include <nano/node/common.hpp>
#include <nano/node/messages.hpp>
#include <nano/node/transport/socket.hpp>

#include <boost/asio/ip/network_v6.hpp>

#include <chrono>
#include <cstdint>

namespace rsnano
{
class BandwidthLimiterHandle;
class ChannelHandle;
class ChannelWeakHandle;
}

namespace nano::transport
{
class callback_visitor : public nano::message_visitor
{
public:
	void keepalive (nano::keepalive const & message_a) override
	{
		result = nano::stat::detail::keepalive;
	}
	void publish (nano::publish const & message_a) override
	{
		result = nano::stat::detail::publish;
	}
	void confirm_req (nano::confirm_req const & message_a) override
	{
		result = nano::stat::detail::confirm_req;
	}
	void confirm_ack (nano::confirm_ack const & message_a) override
	{
		result = nano::stat::detail::confirm_ack;
	}
	void bulk_pull (nano::bulk_pull const & message_a) override
	{
		result = nano::stat::detail::bulk_pull;
	}
	void bulk_pull_account (nano::bulk_pull_account const & message_a) override
	{
		result = nano::stat::detail::bulk_pull_account;
	}
	void bulk_push (nano::bulk_push const & message_a) override
	{
		result = nano::stat::detail::bulk_push;
	}
	void frontier_req (nano::frontier_req const & message_a) override
	{
		result = nano::stat::detail::frontier_req;
	}
	void node_id_handshake (nano::node_id_handshake const & message_a) override
	{
		result = nano::stat::detail::node_id_handshake;
	}
	void telemetry_req (nano::telemetry_req const & message_a) override
	{
		result = nano::stat::detail::telemetry_req;
	}
	void telemetry_ack (nano::telemetry_ack const & message_a) override
	{
		result = nano::stat::detail::telemetry_ack;
	}
	nano::stat::detail result;
};

enum class transport_type : uint8_t
{
	undefined = 0,
	tcp = 1,
	loopback = 2,
	fake = 3
};

class channel
{
public:
	channel (rsnano::ChannelHandle * handle_a);
	channel (nano::transport::channel const &) = delete;
	virtual ~channel ();
	bool is_temporary () const;
	void set_temporary (bool temporary);

	virtual void send (nano::message & message_a,
	std::function<void (boost::system::error_code const &, std::size_t)> const & callback_a = nullptr,
	nano::transport::buffer_drop_policy policy_a = nano::transport::buffer_drop_policy::limiter,
	nano::transport::traffic_type = nano::transport::traffic_type::generic)
	= 0;

	void close ();

	virtual std::string to_string () const = 0;
	virtual nano::endpoint get_remote_endpoint () const = 0;
	virtual nano::tcp_endpoint get_tcp_remote_endpoint () const = 0;
	virtual nano::tcp_endpoint get_local_endpoint () const = 0;
	virtual nano::transport::transport_type get_type () const = 0;

	virtual bool max (nano::transport::traffic_type = nano::transport::traffic_type::generic)
	{
		return false;
	}

	virtual bool alive () const
	{
		return true;
	}

	std::chrono::system_clock::time_point get_last_bootstrap_attempt () const;
	void set_last_bootstrap_attempt ();

	std::chrono::system_clock::time_point get_last_packet_received () const;
	void set_last_packet_received ();

	std::chrono::system_clock::time_point get_last_packet_sent () const;
	void set_last_packet_sent ();
	void set_last_packet_sent (std::chrono::system_clock::time_point time);

	boost::optional<nano::account> get_node_id_optional () const;
	nano::account get_node_id () const;
	void set_node_id (nano::account node_id_a);

	size_t channel_id () const;

	virtual uint8_t get_network_version () const = 0;
	virtual nano::endpoint get_peering_endpoint () const = 0;
	virtual void set_peering_endpoint (nano::endpoint endpoint) = 0;
	rsnano::ChannelHandle * handle;
};

class channel_weak_ptr
{
public:
	channel_weak_ptr (const std::shared_ptr<nano::transport::channel> & channel_a);
	channel_weak_ptr (const channel_weak_ptr &) = delete;
	channel_weak_ptr (channel_weak_ptr &&);
	~channel_weak_ptr ();
	std::shared_ptr<nano::transport::channel> upgrade () const;

private:
	rsnano::ChannelWeakHandle * handle;
};
}
