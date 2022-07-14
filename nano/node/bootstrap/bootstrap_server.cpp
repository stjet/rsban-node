#include <nano/lib/rsnanoutils.hpp>
#include <nano/node/bootstrap/bootstrap_bulk_push.hpp>
#include <nano/node/bootstrap/bootstrap_frontier.hpp>
#include <nano/node/bootstrap/bootstrap_server.hpp>
#include <nano/node/node.hpp>
#include <nano/node/transport/tcp.hpp>

#include <boost/format.hpp>
#include <boost/variant/get.hpp>

nano::bootstrap_server_weak_wrapper::bootstrap_server_weak_wrapper (std::shared_ptr<nano::bootstrap_server> const & server) :
	handle{ rsnano::rsn_bootstrap_server_get_weak (server->handle) }
{
}

nano::bootstrap_server_weak_wrapper::bootstrap_server_weak_wrapper (bootstrap_server_weak_wrapper const & other_a) :
	handle{ rsnano::rsn_bootstrap_server_copy_weak (other_a.handle) }
{
}

nano::bootstrap_server_weak_wrapper::bootstrap_server_weak_wrapper (bootstrap_server_weak_wrapper && other_a) noexcept :
	handle{ other_a.handle }
{
	other_a.handle = nullptr;
}

nano::bootstrap_server_weak_wrapper::~bootstrap_server_weak_wrapper ()
{
	if (handle)
		rsnano::rsn_bootstrap_server_destroy_weak (handle);
}

nano::bootstrap_server_weak_wrapper & nano::bootstrap_server_weak_wrapper::operator= (bootstrap_server_weak_wrapper && other_a) noexcept
{
	handle = other_a.handle;
	other_a.handle = nullptr;
	return *this;
}

std::shared_ptr<nano::bootstrap_server> nano::bootstrap_server_weak_wrapper::lock () const
{
	auto server_handle = rsnano::rsn_bootstrap_server_lock_weak (handle);
	if (server_handle)
		return std::make_shared<nano::bootstrap_server> (server_handle);

	return std::shared_ptr<nano::bootstrap_server> ();
}

nano::bootstrap_listener::bootstrap_listener (uint16_t port_a, nano::node & node_a) :
	node (node_a),
	port (port_a)
{
}

void nano::bootstrap_listener::start ()
{
	nano::lock_guard<nano::mutex> lock (mutex);
	on = true;
	listening_socket = std::make_shared<nano::server_socket> (node, boost::asio::ip::tcp::endpoint (boost::asio::ip::address_v6::any (), port), node.config->tcp_incoming_connections_max);
	boost::system::error_code ec;
	listening_socket->start (ec);
	if (ec)
	{
		node.logger->always_log (boost::str (boost::format ("Network: Error while binding for incoming TCP/bootstrap on port %1%: %2%") % listening_socket->listening_port () % ec.message ()));
		throw std::runtime_error (ec.message ());
	}

	// the user can either specify a port value in the config or it can leave the choice up to the OS;
	// independently of user's port choice, he may have also opted to disable UDP or not; this gives us 4 possibilities:
	// (1): UDP enabled, port specified
	// (2): UDP enabled, port not specified
	// (3): UDP disabled, port specified
	// (4): UDP disabled, port not specified
	//
	const auto listening_port = listening_socket->listening_port ();
	if (!node.flags.disable_udp)
	{
		// (1) and (2) -- no matter if (1) or (2), since UDP socket binding happens before this TCP socket binding,
		// we must have already been constructed with a valid port value, so check that it really is the same everywhere
		//
		debug_assert (port == listening_port);
		debug_assert (port == node.network.port);
		debug_assert (port == node.network.endpoint ().port ());
	}
	else
	{
		// (3) -- nothing to do, just check that port values match everywhere
		//
		if (port == listening_port)
		{
			debug_assert (port == node.network.port);
			debug_assert (port == node.network.endpoint ().port ());
		}
		// (4) -- OS port choice happened at TCP socket bind time, so propagate this port value back;
		// the propagation is done here for the `bootstrap_listener` itself, whereas for `network`, the node does it
		// after calling `bootstrap_listener.start ()`
		//
		else
		{
			port = listening_port;
		}
	}

	listening_socket->on_connection ([this] (std::shared_ptr<nano::socket> const & new_connection, boost::system::error_code const & ec_a) {
		if (!ec_a)
		{
			accept_action (ec_a, new_connection);
		}
		return true;
	});
}

void nano::bootstrap_listener::stop ()
{
	decltype (connections) connections_l;
	{
		nano::lock_guard<nano::mutex> lock (mutex);
		on = false;
		connections_l.swap (connections);
	}
	if (listening_socket)
	{
		nano::lock_guard<nano::mutex> lock (mutex);
		listening_socket->close ();
		listening_socket = nullptr;
	}
}

std::size_t nano::bootstrap_listener::connection_count ()
{
	nano::lock_guard<nano::mutex> lock (mutex);
	return connections.size ();
}

void nano::bootstrap_listener::erase_connection (std::uintptr_t conn_ptr)
{
	nano::lock_guard<nano::mutex> lock (mutex);
	connections.erase (conn_ptr);
}

std::size_t nano::bootstrap_listener::get_bootstrap_count ()
{
	return bootstrap_count;
}

void nano::bootstrap_listener::inc_bootstrap_count ()
{
	++bootstrap_count;
}

void nano::bootstrap_listener::dec_bootstrap_count ()
{
	--bootstrap_count;
}

std::size_t nano::bootstrap_listener::get_realtime_count ()
{
	return realtime_count;
}

void nano::bootstrap_listener::inc_realtime_count ()
{
	++realtime_count;
}

void nano::bootstrap_listener::dec_realtime_count ()
{
	--realtime_count;
}

void nano::bootstrap_listener::bootstrap_server_timeout (std::uintptr_t inner_ptr)
{
	if (node.config->logging.bulk_pull_logging ())
	{
		node.logger->try_log ("Closing incoming tcp / bootstrap server by timeout");
	}
	{
		erase_connection (inner_ptr);
	}
}

void nano::bootstrap_listener::boostrap_server_exited (nano::socket::type_t type_a, std::uintptr_t inner_ptr_a, nano::tcp_endpoint const & endpoint_a)
{
	if (node.config->logging.bulk_pull_logging ())
	{
		node.logger->try_log ("Exiting incoming TCP/bootstrap server");
	}
	if (type_a == nano::socket::type_t::bootstrap)
	{
		dec_bootstrap_count ();
	}
	else if (type_a == nano::socket::type_t::realtime)
	{
		dec_realtime_count ();
		// Clear temporary channel
		node.network.tcp_channels->erase_temporary_channel (endpoint_a);
	}
	erase_connection (inner_ptr_a);
}

void nano::bootstrap_listener::accept_action (boost::system::error_code const & ec, std::shared_ptr<nano::socket> const & socket_a)
{
	if (!node.network.excluded_peers.check (socket_a->remote_endpoint ()))
	{
		auto connection (std::make_shared<nano::bootstrap_server> (socket_a, node.shared ()));
		nano::lock_guard<nano::mutex> lock (mutex);
		connections[connection->unique_id ()] = nano::bootstrap_server_weak_wrapper (connection);
		connection->receive ();
	}
	else
	{
		node.stats->inc (nano::stat::type::tcp, nano::stat::detail::tcp_excluded);
		if (node.config->logging.network_rejected_logging ())
		{
			node.logger->try_log ("Rejected connection from excluded peer ", socket_a->remote_endpoint ());
		}
	}
}

boost::asio::ip::tcp::endpoint nano::bootstrap_listener::endpoint ()
{
	nano::lock_guard<nano::mutex> lock (mutex);
	if (on && listening_socket)
	{
		return boost::asio::ip::tcp::endpoint (boost::asio::ip::address_v6::loopback (), port);
	}
	else
	{
		return boost::asio::ip::tcp::endpoint (boost::asio::ip::address_v6::loopback (), 0);
	}
}

std::unique_ptr<nano::container_info_component> nano::collect_container_info (bootstrap_listener & bootstrap_listener, std::string const & name)
{
	//auto sizeof_element = sizeof (decltype (bootstrap_listener.connections)::value_type);
	size_t sizeof_element = 1;
	auto composite = std::make_unique<container_info_composite> (name);
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "connections", bootstrap_listener.connection_count (), sizeof_element }));
	return composite;
}

nano::bootstrap_server_lock::bootstrap_server_lock (rsnano::BootstrapServerLockHandle * handle_a) :
	handle{ handle_a }
{
}

nano::bootstrap_server_lock::bootstrap_server_lock (bootstrap_server_lock const & other_a) :
	handle{ rsnano::rsn_bootstrap_server_lock_clone (other_a.handle) }
{
}

nano::bootstrap_server_lock::bootstrap_server_lock (bootstrap_server_lock && other_a) noexcept :
	handle{ other_a.handle }
{
	other_a.handle = nullptr;
}

nano::bootstrap_server_lock::~bootstrap_server_lock ()
{
	if (handle)
		rsnano::rsn_bootstrap_server_lock_destroy (handle);
}

nano::locked_bootstrap_server_requests::locked_bootstrap_server_requests (nano::bootstrap_server_lock lock_a) :
	lock{ lock_a }
{
}

nano::message * nano::locked_bootstrap_server_requests::release_front_request ()
{
	auto msg_handle{ rsnano::rsn_bootstrap_server_release_front_request (lock.handle) };
	auto message{ nano::message_handle_to_message (msg_handle) };
	return message.release ();
}

nano::bootstrap_server::bootstrap_server (std::shared_ptr<nano::socket> const & socket_a, std::shared_ptr<nano::node> const & node_a)
{
	auto config_dto{ node_a->config->to_dto () };
	auto observer_handle = new std::shared_ptr<nano::bootstrap_server_observer> (node_a->bootstrap);
	auto network_dto{ node_a->network_params.to_dto () };
	rsnano::io_ctx_wrapper io_ctx (node_a->io_ctx);
	auto request_response_visitor_factory{ std::make_shared<nano::request_response_visitor_factory> (node_a) };
	rsnano::CreateBootstrapServerParams params;
	params.socket = socket_a->handle;
	params.config = &config_dto;
	params.logger = nano::to_logger_handle (node_a->logger);
	params.observer = observer_handle;
	params.publish_filter = node_a->network.publish_filter->handle;
	params.workers = new std::shared_ptr<nano::thread_pool> (node_a->workers);
	params.io_ctx = io_ctx.handle ();
	params.network = &network_dto;
	params.disable_bootstrap_listener = node_a->flags.disable_bootstrap_listener;
	params.connections_max = node_a->config->bootstrap_connections_max;
	params.stats = node_a->stats->handle;
	params.disable_bootstrap_bulk_pull_server = node_a->flags.disable_bootstrap_bulk_pull_server;
	params.disable_tcp_realtime = node_a->flags.disable_tcp_realtime;
	params.request_response_visitor_factory = new std::shared_ptr<nano::request_response_visitor_factory> (request_response_visitor_factory);
	handle = rsnano::rsn_bootstrap_server_create (&params);
	debug_assert (socket_a != nullptr);
}

nano::bootstrap_server::bootstrap_server (rsnano::BootstrapServerHandle * handle_a) :
	handle{ handle_a }
{
}

nano::bootstrap_server::~bootstrap_server ()
{
	rsnano::rsn_bootstrap_server_destroy (handle);
}

void nano::bootstrap_server::stop ()
{
	rsnano::rsn_bootstrap_server_stop (handle);
}

void nano::bootstrap_server::receive ()
{
	rsnano::rsn_bootstrap_server_receive (handle);
}

void nano::bootstrap_server::finish_request ()
{
	rsnano::rsn_bootstrap_server_finish_request (handle);
}

void nano::bootstrap_server::finish_request_async ()
{
	rsnano::rsn_bootstrap_server_finish_request_async (handle);
}

bool nano::bootstrap_server::get_handshake_query_received ()
{
	return rsnano::rsn_bootstrap_server_handshake_query_received (handle);
}

void nano::bootstrap_server::set_handshake_query_received ()
{
	rsnano::rsn_bootstrap_server_set_handshake_query_received (handle);
}

void nano::bootstrap_server::timeout ()
{
	rsnano::rsn_bootstrap_server_timeout (handle);
}

void nano::bootstrap_server::push_request (std::unique_ptr<nano::message> msg)
{
	rsnano::MessageHandle * msg_handle = nullptr;
	if (msg)
	{
		msg_handle = msg->handle;
	}
	rsnano::rsn_bootstrap_server_push_request (handle, msg_handle);
}

bool nano::bootstrap_server::requests_empty ()
{
	return rsnano::rsn_bootstrap_server_requests_empty (handle);
}

nano::locked_bootstrap_server_requests::locked_bootstrap_server_requests (nano::locked_bootstrap_server_requests && other_a) noexcept :
	lock{ std::move (other_a.lock) }
{
}

namespace
{
class request_response_visitor : public nano::message_visitor
{
public:
	explicit request_response_visitor (std::shared_ptr<nano::bootstrap_server> connection_a, std::shared_ptr<nano::node> node_a, nano::locked_bootstrap_server_requests & requests_a) :
		connection (std::move (connection_a)),
		node (std::move (node_a)),
		requests{ std::move (requests_a) }
	{
	}
	void keepalive (nano::keepalive const & message_a) override
	{
		node->network.tcp_message_manager.put_message (nano::tcp_message_item{ std::make_shared<nano::keepalive> (message_a), connection->get_remote_endpoint (), connection->get_remote_node_id (), connection->get_socket () });
	}
	void publish (nano::publish const & message_a) override
	{
		node->network.tcp_message_manager.put_message (nano::tcp_message_item{ std::make_shared<nano::publish> (message_a), connection->get_remote_endpoint (), connection->get_remote_node_id (), connection->get_socket () });
	}
	void confirm_req (nano::confirm_req const & message_a) override
	{
		node->network.tcp_message_manager.put_message (nano::tcp_message_item{ std::make_shared<nano::confirm_req> (message_a), connection->get_remote_endpoint (), connection->get_remote_node_id (), connection->get_socket () });
	}
	void confirm_ack (nano::confirm_ack const & message_a) override
	{
		node->network.tcp_message_manager.put_message (nano::tcp_message_item{ std::make_shared<nano::confirm_ack> (message_a), connection->get_remote_endpoint (), connection->get_remote_node_id (), connection->get_socket () });
	}

	// connection.requests still locked and message still in front of queue!:
	//----------------------------------------
	void bulk_pull (nano::bulk_pull const &) override
	{
		auto response (std::make_shared<nano::bulk_pull_server> (node, connection, std::unique_ptr<nano::bulk_pull> (static_cast<nano::bulk_pull *> (requests.release_front_request ()))));
		response->send_next ();
	}
	void bulk_pull_account (nano::bulk_pull_account const &) override
	{
		auto response (std::make_shared<nano::bulk_pull_account_server> (node, connection, std::unique_ptr<nano::bulk_pull_account> (static_cast<nano::bulk_pull_account *> (requests.release_front_request ()))));
		response->send_frontier ();
	}
	void bulk_push (nano::bulk_push const &) override
	{
		auto response (std::make_shared<nano::bulk_push_server> (node, connection));
		response->throttled_receive ();
	}
	void frontier_req (nano::frontier_req const &) override
	{
		auto response (std::make_shared<nano::frontier_req_server> (node, connection, std::unique_ptr<nano::frontier_req> (static_cast<nano::frontier_req *> (requests.release_front_request ()))));
		response->send_next ();
	}
	void node_id_handshake (nano::node_id_handshake const & message_a) override
	{
		// check for multiple handshake messages, there is no reason to receive more than one
		if (message_a.get_query () && connection->get_handshake_query_received ())
		{
			if (node->config->logging.network_node_id_handshake_logging ())
			{
				node->logger->try_log (boost::str (boost::format ("Detected multiple node_id_handshake query from %1%") % connection->get_remote_endpoint ()));
			}
			connection->stop ();
			return;
		}

		connection->set_handshake_query_received ();

		if (node->config->logging.network_node_id_handshake_logging ())
		{
			node->logger->try_log (boost::str (boost::format ("Received node_id_handshake message from %1%") % connection->get_remote_endpoint ()));
		}

		if (message_a.get_query ())
		{
			boost::optional<std::pair<nano::account, nano::signature>> response (std::make_pair (node->node_id.pub, nano::sign_message (node->node_id.prv, node->node_id.pub, *message_a.get_query ())));
			debug_assert (!nano::validate_message (response->first, *message_a.get_query (), response->second));
			auto cookie (node->network.syn_cookies.assign (nano::transport::map_tcp_to_endpoint (connection->get_remote_endpoint ())));
			nano::node_id_handshake response_message (node->network_params.network, cookie, response);
			auto shared_const_buffer = response_message.to_shared_const_buffer ();
			connection->get_socket ()->async_write (shared_const_buffer, [connection = nano::bootstrap_server_weak_wrapper (connection), config_l = node->config, stats_l = node->stats, logger_l = node->logger] (boost::system::error_code const & ec, std::size_t size_a) {
				if (auto connection_l = connection.lock ())
				{
					if (ec)
					{
						if (config_l->logging.network_node_id_handshake_logging ())
						{
							logger_l->try_log (boost::str (boost::format ("Error sending node_id_handshake to %1%: %2%") % connection_l->get_remote_endpoint () % ec.message ()));
						}
						// Stop invalid handshake
						connection_l->stop ();
					}
					else
					{
						stats_l->inc (nano::stat::type::message, nano::stat::detail::node_id_handshake, nano::stat::dir::out);
						connection_l->finish_request ();
					}
				}
			});
		}
		else if (message_a.get_response ())
		{
			nano::account const & node_id (message_a.get_response ()->first);
			if (!node->network.syn_cookies.validate (nano::transport::map_tcp_to_endpoint (connection->get_remote_endpoint ()), node_id, message_a.get_response ()->second) && node_id != node->node_id.pub)
			{
				connection->set_remote_node_id (node_id);
				connection->get_socket ()->type_set (nano::socket::type_t::realtime);
				node->bootstrap->inc_realtime_count ();
				connection->finish_request_async ();
			}
			else
			{
				// Stop invalid handshake
				connection->stop ();
			}
		}
		else
		{
			connection->finish_request_async ();
		}
		nano::account node_id (connection->get_remote_node_id ());
		nano::socket::type_t type = connection->get_socket ()->type ();
		debug_assert (node_id.is_zero () || type == nano::socket::type_t::realtime);
		node->network.tcp_message_manager.put_message (nano::tcp_message_item{ std::make_shared<nano::node_id_handshake> (message_a), connection->get_remote_endpoint (), connection->get_remote_node_id (), connection->get_socket () });
	}
	//----------------------------------------

	void telemetry_req (nano::telemetry_req const & message_a) override
	{
		node->network.tcp_message_manager.put_message (nano::tcp_message_item{ std::make_shared<nano::telemetry_req> (message_a), connection->get_remote_endpoint (), connection->get_remote_node_id (), connection->get_socket () });
	}
	void telemetry_ack (nano::telemetry_ack const & message_a) override
	{
		node->network.tcp_message_manager.put_message (nano::tcp_message_item{ std::make_shared<nano::telemetry_ack> (message_a), connection->get_remote_endpoint (), connection->get_remote_node_id (), connection->get_socket () });
	}
	std::shared_ptr<nano::bootstrap_server> connection;
	std::shared_ptr<nano::node> node;
	nano::locked_bootstrap_server_requests requests;
};
}

nano::request_response_visitor_factory::request_response_visitor_factory (std::shared_ptr<nano::node> node_a) :
	node{ std::move (node_a) }
{
}

std::shared_ptr<nano::message_visitor> nano::request_response_visitor_factory::create_visitor (std::shared_ptr<nano::bootstrap_server> connection_a, nano::locked_bootstrap_server_requests & requests)
{
	return std::make_shared<request_response_visitor> (connection_a, node, requests);
}

bool nano::bootstrap_server::is_stopped () const
{
	return rsnano::rsn_bootstrap_server_is_stopped (handle);
}

std::uintptr_t nano::bootstrap_server::unique_id () const
{
	return rsnano::rsn_bootstrap_server_unique_id (handle);
}

nano::account nano::bootstrap_server::get_remote_node_id () const
{
	nano::account node_id;
	rsnano::rsn_bootstrap_server_remote_node_id (handle, node_id.bytes.data ());
	return node_id;
}

void nano::bootstrap_server::set_remote_node_id (nano::account account_a)
{
	rsnano::rsn_bootstrap_server_set_remote_node_id (handle, account_a.bytes.data ());
}

nano::tcp_endpoint nano::bootstrap_server::get_remote_endpoint () const
{
	rsnano::EndpointDto dto;
	rsnano::rsn_bootstrap_server_remote_endpoint (handle, &dto);
	return rsnano::dto_to_endpoint (dto);
}

std::shared_ptr<nano::socket> const nano::bootstrap_server::get_socket () const
{
	auto socket_handle = rsnano::rsn_bootstrap_server_socket (handle);
	return std::make_shared<nano::socket> (socket_handle);
}
