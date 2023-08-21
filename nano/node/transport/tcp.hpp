#pragma once

#include "nano/lib/rsnano.hpp"
#include "nano/node/election.hpp"
#include "nano/node/nodeconfig.hpp"

#include <nano/node/common.hpp>
#include <nano/node/peer_exclusion.hpp>
#include <nano/node/transport/channel.hpp>
#include <nano/node/transport/transport.hpp>

#include <chrono>
#include <cstddef>
#include <cstdint>
#include <memory>
#include <unordered_set>

namespace nano
{
class bootstrap_server;
class node_config;
class node_flags;
class network;
class telemetry;
class syn_cookies;
class tcp_server_observer;
class tcp_message_item final
{
public:
	tcp_message_item ();
	explicit tcp_message_item (rsnano::TcpMessageItemHandle * handle_a);
	tcp_message_item (std::shared_ptr<nano::message> message_a, nano::tcp_endpoint endpoint_a, nano::account node_id_a, std::shared_ptr<nano::transport::socket> socket_a);
	tcp_message_item (nano::tcp_message_item const & other_a);
	tcp_message_item (nano::tcp_message_item && other_a) noexcept;
	~tcp_message_item ();
	tcp_message_item & operator= (tcp_message_item const & other_a);
	tcp_message_item & operator= (tcp_message_item && other_a);
	std::shared_ptr<nano::message> get_message () const;
	nano::tcp_endpoint get_endpoint () const;
	nano::account get_node_id () const;
	std::shared_ptr<nano::transport::socket> get_socket () const;
	rsnano::TcpMessageItemHandle * handle;
};

class tcp_message_manager final
{
public:
	explicit tcp_message_manager (unsigned incoming_connections_max_a);
	tcp_message_manager (tcp_message_manager const &) = delete;
	tcp_message_manager (tcp_message_manager &&) = delete;
	~tcp_message_manager ();
	void put_message (nano::tcp_message_item const & item_a);
	nano::tcp_message_item get_message ();
	// Stop container and notify waiting threads
	void stop ();
	rsnano::TcpMessageManagerHandle * handle;
};

namespace transport
{
	class tcp_server;
	class tcp_channels;

	class request_response_visitor_factory
	{
	public:
		explicit request_response_visitor_factory (nano::node & node_a);
		request_response_visitor_factory (request_response_visitor_factory const &) = delete;
		~request_response_visitor_factory ();
		rsnano::RequestResponseVisitorFactoryHandle * handle;
	};

	class channel_tcp_observer
	{
	public:
		virtual void data_sent (boost::asio::ip::tcp::endpoint const & endpoint_a) = 0;
		virtual void host_unreachable () = 0;
		virtual void message_sent (nano::message const & message_a) = 0;
		virtual void message_dropped (nano::message const & message_a, std::size_t buffer_size_a) = 0;
		virtual void no_socket_drop () = 0;
		virtual void write_drop () = 0;
	};

	void channel_tcp_send_callback (void * context_a, const rsnano::ErrorCodeDto * ec_a, std::size_t size_a);
	void delete_send_buffer_callback (void * context_a);

	class channel_tcp : public nano::transport::channel
	{
		friend class nano::transport::tcp_channels;

	public:
		channel_tcp (boost::asio::io_context & io_ctx_a, nano::outbound_bandwidth_limiter & limiter_a, nano::network_constants const & network_a, std::shared_ptr<nano::transport::socket> const & socket_a, std::shared_ptr<nano::transport::channel_tcp_observer> const & observer_a, size_t channel_id);
		channel_tcp (rsnano::ChannelHandle * handle_a) :
			channel{ handle_a } {};

		uint8_t get_network_version () const override;
		void set_network_version (uint8_t network_version_a);
		std::size_t hash_code () const override;
		bool operator== (nano::transport::channel const &) const override;
		void send (nano::message & message_a, std::function<void (boost::system::error_code const &, std::size_t)> const & callback_a = nullptr, nano::transport::buffer_drop_policy policy_a = nano::transport::buffer_drop_policy::limiter, nano::transport::traffic_type = nano::transport::traffic_type::generic) override;

		// TODO: investigate clang-tidy warning about default parameters on virtual/override functions
		void send_buffer (nano::shared_const_buffer const &, std::function<void (boost::system::error_code const &, std::size_t)> const & = nullptr, nano::transport::buffer_drop_policy = nano::transport::buffer_drop_policy::limiter, nano::transport::traffic_type = nano::transport::traffic_type::generic) override;

		std::string to_string () const override;
		bool operator== (nano::transport::channel_tcp const & other_a) const;
		std::shared_ptr<nano::transport::socket> try_get_socket () const;

		void set_endpoint ();

		nano::endpoint get_endpoint () const override
		{
			return nano::transport::map_tcp_to_endpoint (get_tcp_endpoint ());
		}

		nano::tcp_endpoint get_tcp_endpoint () const override;
		nano::transport::transport_type get_type () const override
		{
			return nano::transport::transport_type::tcp;
		}

		bool max (nano::transport::traffic_type traffic_type) override;
		nano::endpoint get_peering_endpoint () const override;
		void set_peering_endpoint (nano::endpoint endpoint) override;
		virtual bool alive () const override;
	};

	class tcp_server_factory
	{
	public:
		tcp_server_factory (
		nano::node_config & config,
		boost::asio::io_context & io_ctx_a,
		std::shared_ptr<nano::logger_mt> const & logger_a,
		nano::network_filter & publish_filter,
		nano::stats & stats_a,
		nano::block_uniquer & block_uniquer_a,
		nano::vote_uniquer & vote_uniquer_a,
		nano::tcp_message_manager & message_manager_a);
		tcp_server_factory (tcp_server_factory const &) = delete;
		tcp_server_factory (tcp_server_factory &&) = delete;
		~tcp_server_factory ();
		void set_observer (std::shared_ptr<nano::tcp_server_observer> observer_a);
		void set_message_visitor_factory (nano::transport::request_response_visitor_factory & visitor_factory);
		std::shared_ptr<nano::transport::tcp_server> create_tcp_server (const std::shared_ptr<nano::transport::channel_tcp> & channel_a, const std::shared_ptr<nano::transport::socket> & socket_a);
		rsnano::TcpServerFactoryHandle * handle;
	};

	class tcp_channels final : public nano::transport::channel_tcp_observer, public std::enable_shared_from_this<tcp_channels>
	{
		friend class nano::transport::channel_tcp;

	public:
		explicit tcp_channels (nano::node &, uint16_t port, std::function<void (nano::message const &, std::shared_ptr<nano::transport::channel> const &)> = nullptr);
		tcp_channels (nano::transport::tcp_channels const &) = delete;
		~tcp_channels ();
		bool insert (std::shared_ptr<nano::transport::channel_tcp> const &, std::shared_ptr<nano::transport::socket> const &, std::shared_ptr<nano::transport::tcp_server> const &);
		void erase (nano::tcp_endpoint const &);
		void erase_temporary_channel (nano::tcp_endpoint const &);
		std::size_t size () const;
		std::shared_ptr<nano::transport::channel_tcp> find_channel (nano::tcp_endpoint const &) const;
		std::vector<std::shared_ptr<nano::transport::channel>> random_channels (std::size_t, uint8_t = 0, bool = false) const;
		std::vector<endpoint> get_current_peers () const;
		std::shared_ptr<nano::transport::channel_tcp> find_node_id (nano::account const &);
		// Get the next peer for attempting a tcp connection
		nano::tcp_endpoint bootstrap_peer ();
		void receive ();
		void start ();
		void stop ();
		bool not_a_peer (nano::endpoint const &, bool);
		void process_messages ();
		void process_message (nano::message const &, nano::tcp_endpoint const &, nano::account const &, std::shared_ptr<nano::transport::socket> const &);
		bool max_ip_connections (nano::tcp_endpoint const & endpoint_a);
		bool max_subnetwork_connections (nano::tcp_endpoint const & endpoint_a);
		bool max_ip_or_subnetwork_connections (nano::tcp_endpoint const & endpoint_a);
		// Should we reach out to this endpoint with a keepalive message
		bool reachout (nano::endpoint const &);
		std::unique_ptr<container_info_component> collect_container_info (std::string const &);
		void purge (std::chrono::system_clock::time_point const &);
		void ongoing_keepalive ();
		void list (std::deque<std::shared_ptr<nano::transport::channel>> &, uint8_t = 0, bool = true);
		void modify_last_packet_sent (nano::endpoint const & endpoint_a, std::chrono::system_clock::time_point const & time_a);
		void update (nano::tcp_endpoint const &);
		// Connection start
		void start_tcp (nano::endpoint const &);
		void start_tcp_receive_node_id (std::shared_ptr<nano::transport::channel_tcp> const &, nano::endpoint const &, std::shared_ptr<std::vector<uint8_t>> const &);
		void on_new_channel (std::function<void (std::shared_ptr<nano::transport::channel>)> observer_a);

		// channel_tcp_observer:
		void data_sent (boost::asio::ip::tcp::endpoint const & endpoint_a) override;
		void host_unreachable () override;
		void message_sent (nano::message const & message_a) override;
		void message_dropped (nano::message const & message_a, std::size_t buffer_size_a) override;
		void no_socket_drop () override;
		void write_drop () override;
		std::vector<nano::endpoint> get_peers () const;
		void random_fill (std::array<nano::endpoint, 8> &) const;
		void set_port (uint16_t port_a);
		void set_observer (std::shared_ptr<nano::tcp_server_observer> observer_a);
		void set_message_visitor_factory (nano::transport::request_response_visitor_factory & visitor_factory);
		std::shared_ptr<nano::transport::channel_tcp> get_first_channel () const;

		nano::tcp_message_manager tcp_message_manager;
		nano::peer_exclusion excluded_peers;
		std::shared_ptr<nano::network_filter> publish_filter;
		std::atomic<size_t> next_channel_id{ 1 };

	private:
		std::optional<nano::node_id_handshake::query_payload> prepare_handshake_query (nano::endpoint const & remote_endpoint);
		nano::node_id_handshake::response_payload prepare_handshake_response (nano::node_id_handshake::query_payload const & query, bool v2) const;
		/** Verifies that handshake response matches our query. @returns true if OK */
		bool verify_handshake_response (nano::node_id_handshake::response_payload const & response, nano::endpoint const & remote_endpoint);
		std::function<void (nano::message const &, std::shared_ptr<nano::transport::channel> const &)> sink;

		class channel_tcp_wrapper final
		{
		public:
			channel_tcp_wrapper (std::shared_ptr<nano::transport::channel_tcp> channel_a, std::shared_ptr<nano::transport::socket> socket_a, std::shared_ptr<nano::transport::tcp_server> server_a);
			channel_tcp_wrapper (rsnano::ChannelTcpWrapperHandle * handle_a);
			channel_tcp_wrapper (channel_tcp_wrapper const &) = delete;
			~channel_tcp_wrapper ();
			std::shared_ptr<nano::transport::channel_tcp> get_channel () const;
			std::shared_ptr<nano::transport::tcp_server> get_response_server () const;
			nano::tcp_endpoint endpoint () const
			{
				return get_channel ()->get_tcp_endpoint ();
			}
			uint64_t last_packet_sent () const
			{
				return get_channel ()->get_last_packet_sent ();
			}
			uint64_t last_bootstrap_attempt () const
			{
				return get_channel ()->get_last_bootstrap_attempt ();
			}
			std::shared_ptr<nano::transport::socket> try_get_socket () const
			{
				return get_channel ()->try_get_socket ();
			}
			boost::asio::ip::address ip_address () const
			{
				return nano::transport::ipv4_address_or_ipv6_subnet (endpoint ().address ());
			}
			boost::asio::ip::address subnetwork () const
			{
				return nano::transport::map_address_to_subnetwork (endpoint ().address ());
			}
			nano::account node_id () const
			{
				auto node_id (get_channel ()->get_node_id ());
				return node_id;
			}
			uint8_t network_version () const
			{
				return get_channel ()->get_network_version ();
			}

			rsnano::ChannelTcpWrapperHandle * handle;
		};
		class tcp_endpoint_attempt final
		{
		public:
			nano::tcp_endpoint endpoint;
			boost::asio::ip::address address;
			boost::asio::ip::address subnetwork;
			std::chrono::system_clock::time_point last_attempt{ std::chrono::system_clock::now () };

			explicit tcp_endpoint_attempt (nano::tcp_endpoint const & endpoint_a) :
				endpoint (endpoint_a),
				address (nano::transport::ipv4_address_or_ipv6_subnet (endpoint_a.address ())),
				subnetwork (nano::transport::map_address_to_subnetwork (endpoint_a.address ()))
			{
			}
		};
		nano::keypair node_id;
		nano::network_params & network_params;
		nano::outbound_bandwidth_limiter & limiter;
		std::shared_ptr<nano::syn_cookies> syn_cookies;
		std::shared_ptr<nano::stats> stats;
		std::shared_ptr<nano::node_config> config;
		std::shared_ptr<nano::logger_mt> logger;
		std::shared_ptr<nano::thread_pool> workers;
		std::shared_ptr<nano::node_observers> observers;
		nano::store & store;
		nano::node_flags flags;
		nano::node & node;
		boost::asio::io_context & io_ctx;
		mutable nano::mutex mutex;

	private:
		std::atomic<bool> stopped{ false };
		// Called when a new channel is observed
		std::function<void (std::shared_ptr<nano::transport::channel>)> channel_observer;
		uint16_t port;
		nano::transport::tcp_server_factory tcp_server_factory;
		rsnano::TcpChannelsHandle * handle;

		friend class network_peer_max_tcp_attempts_subnetwork_Test;
	};

	std::shared_ptr<nano::transport::channel> channel_handle_to_channel (rsnano::ChannelHandle * handle);
} // namespace transport
} // namespace nano
