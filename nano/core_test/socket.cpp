#include <nano/boost/asio/ip/address_v6.hpp>
#include <nano/boost/asio/ip/network_v6.hpp>
#include <nano/lib/thread_runner.hpp>
#include <nano/lib/threading.hpp>
#include <nano/node/inactive_node.hpp>
#include <nano/node/transport/socket.hpp>
#include <nano/node/transport/tcp_listener.hpp>
#include <nano/test_common/system.hpp>
#include <nano/test_common/testutil.hpp>

#include <gtest/gtest.h>

#include <boost/asio/read.hpp>

#include <memory>
#include <vector>

using namespace std::chrono_literals;

TEST (socket, max_connections)
{
	nano::test::system system;
	auto node = system.add_node ();
	auto server_port = system.get_available_port ();

	// successful incoming connections are stored in server_sockets to keep them alive (server side)
	std::vector<std::shared_ptr<nano::transport::socket>> server_sockets;

	// start a server socket that allows max 2 live connections
	nano::transport::tcp_config tcp_config{ nano::dev::network_params.network };
	tcp_config.max_inbound_connections = 2;

	auto listener = std::make_shared<nano::transport::tcp_listener> (server_port, tcp_config, *node);
	nano::test::stop_guard stop_guard{ *listener };
	listener->start ([&server_sockets] (std::shared_ptr<nano::transport::socket> const & new_connection, boost::system::error_code const & ec) {
		if (!ec)
		{
			server_sockets.push_back (new_connection);
		}
		return true;
	});

	boost::asio::ip::tcp::endpoint dst_endpoint{ boost::asio::ip::address_v6::loopback (), listener->endpoint ().port () };

	// client side connection tracking
	std::atomic<size_t> connection_attempts = 0;
	auto connect_handler = [&connection_attempts] (boost::system::error_code const & ec_a) {
		ASSERT_EQ (ec_a.value (), 0);
		++connection_attempts;
	};

	// start 3 clients, 2 will persist but 1 will be dropped

	auto client1 = nano::transport::create_client_socket (*node);
	client1->async_connect (dst_endpoint, connect_handler);

	auto client2 = nano::transport::create_client_socket (*node);
	client2->async_connect (dst_endpoint, connect_handler);

	auto client3 = nano::transport::create_client_socket (*node);
	client3->async_connect (dst_endpoint, connect_handler);

	auto get_tcp_accept_failures = [&node] () {
		return node->stats->count (nano::stat::type::tcp_listener, nano::stat::detail::accept_failure, nano::stat::dir::in);
	};

	auto get_tcp_accept_successes = [&node] () {
		return node->stats->count (nano::stat::type::tcp_listener, nano::stat::detail::accept_success, nano::stat::dir::in);
	};

	ASSERT_TIMELY_EQ (5s, get_tcp_accept_failures (), 1);
	ASSERT_TIMELY_EQ (5s, get_tcp_accept_successes (), 2);
	ASSERT_TIMELY_EQ (5s, connection_attempts, 3);

	// create space for one socket and fill the connections table again

	server_sockets[0].reset ();

	auto client4 = nano::transport::create_client_socket (*node);
	client4->async_connect (dst_endpoint, connect_handler);

	auto client5 = nano::transport::create_client_socket (*node);
	client5->async_connect (dst_endpoint, connect_handler);

	ASSERT_TIMELY_EQ (5s, get_tcp_accept_failures (), 2);
	ASSERT_TIMELY_EQ (5s, get_tcp_accept_successes (), 3);
	ASSERT_TIMELY_EQ (5s, connection_attempts, 5);

	// close all existing sockets and fill the connections table again
	// start counting form 1 because 0 is the already closed socket

	server_sockets[1].reset ();
	server_sockets[2].reset ();
	ASSERT_EQ (server_sockets.size (), 3);

	auto client6 = nano::transport::create_client_socket (*node);
	client6->async_connect (dst_endpoint, connect_handler);

	auto client7 = nano::transport::create_client_socket (*node);
	client7->async_connect (dst_endpoint, connect_handler);

	auto client8 = nano::transport::create_client_socket (*node);
	client8->async_connect (dst_endpoint, connect_handler);

	ASSERT_TIMELY_EQ (5s, get_tcp_accept_failures (), 3);
	ASSERT_TIMELY_EQ (5s, get_tcp_accept_successes (), 5);
	ASSERT_TIMELY_EQ (5s, connection_attempts, 8); // connections initiated by the client
	ASSERT_TIMELY_EQ (5s, server_sockets.size (), 5); // connections accepted by the server
}

TEST (socket, max_connections_per_ip)
{
	nano::test::system system;

	auto node = system.add_node ();
	ASSERT_FALSE (node->flags.disable_max_peers_per_ip ());

	auto server_port = system.get_available_port ();

	const auto max_ip_connections = node->network_params.network.max_peers_per_ip;
	ASSERT_GE (max_ip_connections, 1);

	const auto max_global_connections = 1000;

	// successful incoming connections are stored in server_sockets to keep them alive (server side)
	std::vector<std::shared_ptr<nano::transport::socket>> server_sockets;

	nano::transport::tcp_config tcp_config{ nano::dev::network_params.network };
	tcp_config.max_inbound_connections = max_global_connections;

	auto listener = std::make_shared<nano::transport::tcp_listener> (server_port, tcp_config, *node);
	nano::test::stop_guard stop_guard{ *listener };
	listener->start ([&server_sockets] (std::shared_ptr<nano::transport::socket> const & new_connection, boost::system::error_code const & ec) {
		if (!ec)
		{
			server_sockets.push_back (new_connection);
		}
		return true;
	});

	boost::asio::ip::tcp::endpoint dst_endpoint{ boost::asio::ip::address_v6::loopback (), listener->endpoint ().port () };

	// client side connection tracking
	std::atomic<size_t> connection_attempts = 0;
	auto connect_handler = [&connection_attempts] (boost::system::error_code const & ec_a) {
		ASSERT_EQ (ec_a.value (), 0);
		++connection_attempts;
	};

	// start n clients, n-1 will persist but 1 will be dropped, where n == max_ip_connections
	std::vector<std::shared_ptr<nano::transport::socket>> client_list;
	client_list.reserve (max_ip_connections + 1);

	for (auto idx = 0; idx < max_ip_connections + 1; ++idx)
	{
		auto client = nano::transport::create_client_socket (*node);
		client->async_connect (dst_endpoint, connect_handler);
		client_list.push_back (client);
	}

	auto get_tcp_max_per_ip = [&node] () {
		return node->stats->count (nano::stat::type::tcp_listener, nano::stat::detail::max_per_ip, nano::stat::dir::in);
	};

	auto get_tcp_accept_successes = [&node] () {
		return node->stats->count (nano::stat::type::tcp_listener, nano::stat::detail::accept_success, nano::stat::dir::in);
	};

	ASSERT_TIMELY_EQ (5s, get_tcp_accept_successes (), max_ip_connections);
	ASSERT_TIMELY_EQ (5s, get_tcp_max_per_ip (), 1);
	ASSERT_TIMELY_EQ (5s, connection_attempts, max_ip_connections + 1);
}

TEST (socket, limited_subnet_address)
{
	auto address = boost::asio::ip::make_address ("a41d:b7b2:8298:cf45:672e:bd1a:e7fb:f713");
	auto network = nano::transport::socket_functions::get_ipv6_subnet_address (address.to_v6 (), 32); // network prefix = 32.
	ASSERT_EQ ("a41d:b7b2:8298:cf45:672e:bd1a:e7fb:f713/32", network.to_string ());
	ASSERT_EQ ("a41d:b7b2::/32", network.canonical ().to_string ());
}

TEST (socket, max_connections_per_subnetwork)
{
	nano::test::system system;

	nano::node_flags node_flags;
	// disabling IP limit because it will be used the same IP address to check they come from the same subnetwork.
	node_flags.set_disable_max_peers_per_ip (true);
	node_flags.set_disable_max_peers_per_subnetwork (false);
	auto node = system.add_node (node_flags);
	ASSERT_TRUE (node->flags.disable_max_peers_per_ip ());
	ASSERT_FALSE (node->flags.disable_max_peers_per_subnetwork ());

	auto server_port = system.get_available_port ();
	boost::asio::ip::tcp::endpoint listen_endpoint{ boost::asio::ip::address_v6::any (), server_port };

	const auto max_subnetwork_connections = node->network_params.network.max_peers_per_subnetwork;
	ASSERT_GE (max_subnetwork_connections, 1);

	const auto max_global_connections = 1000;

	// successful incoming connections are stored in server_sockets to keep them alive (server side)
	std::vector<std::shared_ptr<nano::transport::socket>> server_sockets;

	nano::transport::tcp_config tcp_config{ nano::dev::network_params.network };
	tcp_config.max_inbound_connections = max_global_connections;

	auto listener = std::make_shared<nano::transport::tcp_listener> (server_port, tcp_config, *node);
	nano::test::stop_guard stop_guard{ *listener };
	listener->start ([&server_sockets] (std::shared_ptr<nano::transport::socket> const & new_connection, boost::system::error_code const & ec) {
		if (!ec)
		{
			server_sockets.push_back (new_connection);
		}
		return true;
	});

	boost::asio::ip::tcp::endpoint dst_endpoint{ boost::asio::ip::address_v6::loopback (), listener->endpoint ().port () };

	// client side connection tracking
	std::atomic<size_t> connection_attempts = 0;
	auto connect_handler = [&connection_attempts] (boost::system::error_code const & ec_a) {
		ASSERT_EQ (ec_a.value (), 0);
		++connection_attempts;
	};

	// start n clients, n-1 will persist but 1 will be dropped, where n == max_subnetwork_connections
	std::vector<std::shared_ptr<nano::transport::socket>> client_list;
	client_list.reserve (max_subnetwork_connections + 1);

	for (auto idx = 0; idx < max_subnetwork_connections + 1; ++idx)
	{
		auto client = nano::transport::create_client_socket (*node);
		client->async_connect (dst_endpoint, connect_handler);
		client_list.push_back (client);
	}

	auto get_tcp_max_per_subnetwork = [&node] () {
		return node->stats->count (nano::stat::type::tcp_listener, nano::stat::detail::max_per_subnetwork, nano::stat::dir::in);
	};

	auto get_tcp_accept_successes = [&node] () {
		return node->stats->count (nano::stat::type::tcp_listener, nano::stat::detail::accept_success, nano::stat::dir::in);
	};

	ASSERT_TIMELY_EQ (5s, get_tcp_accept_successes (), max_subnetwork_connections);
	ASSERT_TIMELY_EQ (5s, get_tcp_max_per_subnetwork (), 1);
	ASSERT_TIMELY_EQ (5s, connection_attempts, max_subnetwork_connections + 1);
}

TEST (socket, disabled_max_peers_per_ip)
{
	nano::test::system system;

	nano::node_flags node_flags;
	node_flags.set_disable_max_peers_per_ip (true);
	auto node = system.add_node (node_flags);
	ASSERT_TRUE (node->flags.disable_max_peers_per_ip ());

	auto server_port = system.get_available_port ();

	const auto max_ip_connections = node->network_params.network.max_peers_per_ip;
	ASSERT_GE (max_ip_connections, 1);

	const auto max_global_connections = 1000;

	// successful incoming connections are stored in server_sockets to keep them alive (server side)
	std::vector<std::shared_ptr<nano::transport::socket>> server_sockets;

	nano::transport::tcp_config tcp_config{ nano::dev::network_params.network };
	tcp_config.max_inbound_connections = max_global_connections;

	auto server_socket = std::make_shared<nano::transport::tcp_listener> (server_port, tcp_config, *node);
	nano::test::stop_guard stop_guard{ *server_socket };
	server_socket->start ([&server_sockets] (std::shared_ptr<nano::transport::socket> const & new_connection, boost::system::error_code const & ec) {
		if (!ec)
		{
			server_sockets.push_back (new_connection);
		}
		return true;
	});

	boost::asio::ip::tcp::endpoint dst_endpoint{ boost::asio::ip::address_v6::loopback (), server_socket->endpoint ().port () };

	// client side connection tracking
	std::atomic<size_t> connection_attempts = 0;
	auto connect_handler = [&connection_attempts] (boost::system::error_code const & ec_a) {
		ASSERT_EQ (ec_a.value (), 0);
		++connection_attempts;
	};

	// start n clients, n-1 will persist but 1 will be dropped, where n == max_ip_connections
	std::vector<std::shared_ptr<nano::transport::socket>> client_list;
	client_list.reserve (max_ip_connections + 1);

	for (auto idx = 0; idx < max_ip_connections + 1; ++idx)
	{
		auto client = nano::transport::create_client_socket (*node);
		client->async_connect (dst_endpoint, connect_handler);
		client_list.push_back (client);
	}

	auto get_tcp_max_per_ip = [&node] () {
		return node->stats->count (nano::stat::type::tcp_listener, nano::stat::detail::max_per_ip, nano::stat::dir::in);
	};

	auto get_tcp_accept_successes = [&node] () {
		return node->stats->count (nano::stat::type::tcp_listener, nano::stat::detail::accept_success, nano::stat::dir::in);
	};

	ASSERT_TIMELY_EQ (5s, get_tcp_accept_successes (), max_ip_connections + 1);
	ASSERT_TIMELY_EQ (5s, get_tcp_max_per_ip (), 0);
	ASSERT_TIMELY_EQ (5s, connection_attempts, max_ip_connections + 1);
}

TEST (socket, disconnection_of_silent_connections)
{
	// TODO implement again!
}

// Disabled, because it doesn't work with Tokio. The Test expects the async runtime to
// not do anything, so that the drop policy can trigger, but Tokio does make connections/sends
// and that prevents the drop. The test must be rewritten
TEST (DISABLED_socket, drop_policy)
{
	// TODO implement again!
}

/**
 * Check that the socket correctly handles a tcp_io_timeout during tcp connect
 * Steps:
 *   set timeout to one second
 *   do a tcp connect that will block for at least a few seconds at the tcp level
 *   check that the connect returns error and that the correct counters have been incremented
 *
 *   NOTE: it is possible that the O/S has tried to access the IP address 10.255.254.253 before
 *   and has it marked in the routing table as unroutable. In that case this test case will fail.
 *   If this test is run repeadetly the tests fails for this reason because the connection fails
 *   with "No route to host" error instead of a timeout.
 */
TEST (socket_timeout, connect)
{
	// create one node and set timeout to 1 second
	nano::test::system system{};
	auto config{ system.default_config () };
	config.tcp_io_timeout = std::chrono::seconds (1);
	auto node{ system.add_node (config) };

	// try to connect to an IP address that most likely does not exist and will not reply
	// we want the tcp stack to not receive a negative reply, we want it to see silence and to keep trying
	// I use the un-routable IP address 10.255.254.253, which is likely to not exist
	boost::asio::ip::tcp::endpoint endpoint (boost::asio::ip::make_address_v6 ("::ffff:10.255.254.253"), 1234);

	// create a client socket and try to connect to the IP address that will not respond
	auto socket = nano::transport::create_client_socket (*node);
	std::atomic<bool> done = false;
	boost::system::error_code ec;
	socket->async_connect (endpoint, [&ec, &done] (boost::system::error_code const & ec_a) {
		if (ec_a)
		{
			ec = ec_a;
			done = true;
		}
	});

	// check that the callback was called and we got an error
	ASSERT_TIMELY_EQ (6s, done, true);
	ASSERT_TRUE (ec);
	ASSERT_EQ (1, node->stats->count (nano::stat::type::tcp, nano::stat::detail::tcp_connect_error, nano::stat::dir::in));

	// check that the socket was closed due to tcp_io_timeout timeout
	// NOTE: this assert is not guaranteed to be always true, it is only likely that it will be true, we can also get "No route to host"
	// if this test is run repeatedly or in parallel then it is guaranteed to fail due to "No route to host" instead of timeout
	ASSERT_EQ (1, node->stats->count (nano::stat::type::tcp, nano::stat::detail::tcp_io_timeout_drop, nano::stat::dir::out));
}

TEST (socket_timeout, write)
{
	// create one node and set timeout to 1 second
	nano::test::system system{};
	auto config{ system.default_config () };
	config.tcp_io_timeout = std::chrono::seconds (2);
	auto node{ system.add_node (config) };

	// create a server socket
	boost::asio::ip::tcp::endpoint endpoint (boost::asio::ip::address_v6::loopback (), system.get_available_port ());
	boost::asio::ip::tcp::acceptor acceptor (system.async_rt.io_ctx);
	acceptor.open (endpoint.protocol ());
	acceptor.bind (endpoint);
	acceptor.listen (boost::asio::socket_base::max_listen_connections);

	// asynchronously accept an incoming connection and create a newsock and do not receive any data
	boost::asio::ip::tcp::socket newsock (system.async_rt.io_ctx);
	acceptor.async_accept (newsock, [] (boost::system::error_code const & ec_a) {
		EXPECT_FALSE (ec_a);
	});

	// create a client socket and send lots of data to fill the socket queue on the local and remote side
	// eventually, the all tcp queues should fill up and async_write will not be able to progress
	// and the timeout should kick in and close the socket, which will cause the async_write to return an error
	auto socket = nano::transport::create_client_socket (*node, 1024 * 64);
	std::atomic<bool> done = false;
	boost::system::error_code ec;
	socket->async_connect (acceptor.local_endpoint (), [&socket, &ec, &done] (boost::system::error_code const & ec_a) {
		EXPECT_FALSE (ec_a);

		auto buffer = std::make_shared<std::vector<uint8_t>> (128 * 1024);
		for (auto i = 0; i < 1024; ++i)
		{
			socket->async_write (nano::shared_const_buffer{ buffer }, [&ec, &done] (boost::system::error_code const & ec_a, size_t size_a) {
				if (ec_a)
				{
					ec = ec_a;
					done = true;
				}
			});
		}
	});

	// check that the callback was called and we got an error
	ASSERT_TIMELY_EQ (10s, done, true);
	ASSERT_TRUE (ec);
	ASSERT_EQ (1, node->stats->count (nano::stat::type::tcp, nano::stat::detail::tcp_write_error, nano::stat::dir::in));

	// check that the socket was closed due to tcp_io_timeout timeout
	ASSERT_EQ (1, node->stats->count (nano::stat::type::tcp, nano::stat::detail::tcp_io_timeout_drop, nano::stat::dir::out));
}

TEST (socket_timeout, write_overlapped)
{
	// create one node and set timeout to 1 second
	nano::test::system system{};
	auto config{ system.default_config () };
	config.tcp_io_timeout = std::chrono::seconds (2);
	auto node{ system.add_node (config) };

	// create a server socket
	boost::asio::ip::tcp::endpoint endpoint (boost::asio::ip::address_v6::loopback (), system.get_available_port ());
	boost::asio::ip::tcp::acceptor acceptor (system.async_rt.io_ctx);
	acceptor.open (endpoint.protocol ());
	acceptor.bind (endpoint);
	acceptor.listen (boost::asio::socket_base::max_listen_connections);

	// asynchronously accept an incoming connection and read 2 bytes only
	boost::asio::ip::tcp::socket newsock (system.async_rt.io_ctx);
	auto buffer = std::make_shared<std::vector<uint8_t>> (1);
	acceptor.async_accept (newsock, [&newsock, &buffer] (boost::system::error_code const & ec_a) {
		EXPECT_FALSE (ec_a);

		boost::asio::async_read (newsock, boost::asio::buffer (buffer->data (), buffer->size ()), [] (boost::system::error_code const & ec_a, size_t size_a) {
			debug_assert (size_a == 1);
		});
	});

	// create a client socket and send lots of data to fill the socket queue on the local and remote side
	// eventually, the all tcp queues should fill up and async_write will not be able to progress
	// and the timeout should kick in and close the socket, which will cause the async_write to return an error
	auto socket = nano::transport::create_client_socket (*node, 1024 * 64);
	std::atomic<bool> done = false;
	boost::system::error_code ec;
	socket->async_connect (acceptor.local_endpoint (), [&socket, &ec, &done] (boost::system::error_code const & ec_a) {
		EXPECT_FALSE (ec_a);

		auto buffer1 = std::make_shared<std::vector<uint8_t>> (1);
		auto buffer2 = std::make_shared<std::vector<uint8_t>> (128 * 1024);
		socket->async_write (nano::shared_const_buffer{ buffer1 }, [] (boost::system::error_code const & ec_a, size_t size_a) {
			debug_assert (size_a == 1);
		});
		for (auto i = 0; i < 1024; ++i)
		{
			socket->async_write (nano::shared_const_buffer{ buffer2 }, [&ec, &done] (boost::system::error_code const & ec_a, size_t size_a) {
				if (ec_a)
				{
					ec = ec_a;
					done = true;
				}
			});
		}
	});

	// check that the callback was called and we got an error
	ASSERT_TIMELY_EQ (10s, done, true);
	ASSERT_TRUE (ec);
	ASSERT_EQ (1, node->stats->count (nano::stat::type::tcp, nano::stat::detail::tcp_write_error, nano::stat::dir::in));

	// check that the socket was closed due to tcp_io_timeout timeout
	ASSERT_EQ (1, node->stats->count (nano::stat::type::tcp, nano::stat::detail::tcp_io_timeout_drop, nano::stat::dir::out));
}
