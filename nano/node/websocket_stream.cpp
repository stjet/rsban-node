#include <nano/node/websocket_stream.hpp>

#include <boost/asio/bind_executor.hpp>

nano::websocket::stream::stream (socket_type socket_a) :
	ws { std::move(socket_a)},
	strand { ws.get_executor ()}
{
}

[[nodiscard]] boost::asio::strand<boost::asio::io_context::executor_type> & nano::websocket::stream::get_strand ()
{
	return strand;
}

[[nodiscard]] socket_type & nano::websocket::stream::get_socket ()
{
	return ws.next_layer ();
}

void nano::websocket::stream::handshake (std::function<void (boost::system::error_code const & ec)> callback_a)
{
	// Websocket handshake
	ws.async_accept ([callback_a] (boost::system::error_code const & ec) {
		callback_a (ec);
	});
}

void nano::websocket::stream::close (boost::beast::websocket::close_reason const & reason_a, boost::system::error_code & ec_a)
{
	ws.close (reason_a, ec_a);
}

void nano::websocket::stream::async_write (nano::shared_const_buffer const & buffer_a, std::function<void (boost::system::error_code, std::size_t)> callback_a)
{
	ws.async_write (buffer_a, boost::asio::bind_executor (strand, callback_a));
}

void nano::websocket::stream::async_read (boost::beast::multi_buffer & buffer_a, std::function<void (boost::system::error_code, std::size_t)> callback_a)
{
	ws.async_read (buffer_a, boost::asio::bind_executor (strand, callback_a));
}
