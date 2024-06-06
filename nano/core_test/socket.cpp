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
	// TODO implement again!
}

TEST (socket, max_connections_per_ip)
{
	// TODO implement again!
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
	// TODO implement again!
}

TEST (socket, disabled_max_peers_per_ip)
{
	// TODO implement again!
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
