#pragma once

#include <nano/boost/asio/strand.hpp>
#include <nano/boost/beast/core.hpp>
#include <nano/boost/beast/websocket.hpp>
#include <nano/lib/asio.hpp>

using socket_type = boost::asio::basic_stream_socket<boost::asio::ip::tcp, boost::asio::io_context::executor_type>;
#define beast_buffers boost::beast::make_printable
using ws_type = boost::beast::websocket::stream<socket_type>;

namespace nano::websocket
{
/**
 * Beast websockets doesn't provide a common base type for tls and non-tls streams, so we use
 * the type erasure idiom to be able to use both kinds of streams through a common type.
 */
class stream final 
{
public:
	stream (socket_type socket_a);
	virtual ~stream () = default;

	[[nodiscard]] virtual boost::asio::strand<boost::asio::io_context::executor_type> & get_strand ();
	[[nodiscard]] virtual socket_type & get_socket ();
	virtual void handshake (std::function<void (boost::system::error_code const & ec)> callback_a);
	virtual void close (boost::beast::websocket::close_reason const & reason_a, boost::system::error_code & ec_a);
	virtual void async_write (nano::shared_const_buffer const & buffer_a, std::function<void (boost::system::error_code, std::size_t)> callback_a);
	virtual void async_read (boost::beast::multi_buffer & buffer_a, std::function<void (boost::system::error_code, std::size_t)> callback_a);

private:
	ws_type ws;
	boost::asio::strand<boost::asio::io_context::executor_type> strand;
};
}
