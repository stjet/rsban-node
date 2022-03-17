#include <nano/boost/asio/ip/address_v6.hpp>
#include <nano/lib/tomlconfig.hpp>
#include <nano/node/websocketconfig.hpp>

nano::websocket::config::config (nano::network_constants & network_constants) :
	network_constants{ network_constants }
{
	rsnano::WebsocketConfigDto dto;
	auto network_dto{ network_constants.to_dto () };
	if (rsnano::rsn_websocket_config_create (&dto, &network_dto) < 0)
		throw std::runtime_error ("could not create websocket config");
	enabled = dto.enabled;
	port = dto.port;
	address = std::string (reinterpret_cast<const char *> (dto.address), dto.address_len);
}

void nano::websocket::config::load_dto (rsnano::WebsocketConfigDto & dto)
{
	enabled = dto.enabled;
	port = dto.port;
	address = std::string (reinterpret_cast<const char *> (dto.address), dto.address_len);
}

rsnano::WebsocketConfigDto nano::websocket::config::to_dto () const
{
	rsnano::WebsocketConfigDto dto;
	dto.enabled = enabled;
	dto.port = port;
	std::copy (address.begin (), address.end (), std::begin (dto.address));
	dto.address_len = address.size ();
	return dto;
}

nano::error nano::websocket::config::deserialize_toml (nano::tomlconfig & toml)
{
	toml.get<bool> ("enable", enabled);
	boost::asio::ip::address_v6 address_l;
	toml.get_optional<boost::asio::ip::address_v6> ("address", address_l, boost::asio::ip::address_v6::loopback ());
	address = address_l.to_string ();
	toml.get<uint16_t> ("port", port);
	return toml.get_error ();
}
