#pragma once

#include <nano/boost/asio/ip/tcp.hpp>
#include <nano/boost/asio/ip/udp.hpp>
#include <nano/lib/rsnano.hpp>

namespace rsnano
{
boost::system::error_code dto_to_error_code (rsnano::ErrorCodeDto const & dto);
rsnano::ErrorCodeDto error_code_to_dto (boost::system::error_code const & ec);
rsnano::EndpointDto udp_endpoint_to_dto (boost::asio::ip::udp::endpoint const & ep);
rsnano::EndpointDto endpoint_to_dto (boost::asio::ip::tcp::endpoint const & ep);
boost::asio::ip::tcp::endpoint dto_to_endpoint (rsnano::EndpointDto const & dto);
boost::asio::ip::udp::endpoint dto_to_udp_endpoint (rsnano::EndpointDto const & dto);
std::string convert_dto_to_string (rsnano::StringDto & dto);
}