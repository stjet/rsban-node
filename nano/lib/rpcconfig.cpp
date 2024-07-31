#include <nano/boost/asio/ip/address_v6.hpp>
#include <nano/lib/config.hpp>
#include <nano/lib/rpcconfig.hpp>
#include <nano/lib/tomlconfig.hpp>

#include <boost/dll/runtime_symbol_info.hpp>

nano::rpc_config::rpc_config (nano::network_constants & network_constants) :
	rpc_process{ network_constants }
{
	rsnano::RpcConfigDto dto;
	auto network_dto{ network_constants.to_dto () };
	if (rsnano::rsn_rpc_config_create (&dto, &network_dto) < 0)
		throw std::runtime_error ("could not create rpc_config");
	load_dto (dto);
}

nano::rpc_config::rpc_config (nano::network_constants & network_constants, uint16_t port_a, bool enable_control_a) :
	rpc_process{ network_constants }
{
	rsnano::RpcConfigDto dto;
	auto network_dto{ network_constants.to_dto () };
	if (rsnano::rsn_rpc_config_create2 (&dto, &network_dto, port_a, enable_control_a) < 0)
		throw std::runtime_error ("could not create rpc_config");
	load_dto (dto);
}

void nano::rpc_config::load_dto (rsnano::RpcConfigDto & dto)
{
	address = std::string (reinterpret_cast<const char *> (dto.address), dto.address_len);
	port = dto.port;
	enable_control = dto.enable_control;
	max_json_depth = dto.max_json_depth;
	max_request_size = dto.max_request_size;
	rpc_logging.log_rpc = dto.rpc_log;
	rpc_process.io_threads = dto.rpc_process.io_threads;
	rpc_process.ipc_address = std::string (reinterpret_cast<const char *> (dto.rpc_process.ipc_address), dto.rpc_process.ipc_address_len);
	rpc_process.ipc_port = dto.rpc_process.ipc_port;
	rpc_process.num_ipc_connections = dto.rpc_process.num_ipc_connections;
}

rsnano::RpcConfigDto nano::rpc_config::to_dto () const
{
	rsnano::RpcConfigDto dto;
	std::copy (address.begin (), address.end (), std::begin (dto.address));
	dto.address_len = address.size ();
	dto.port = port;
	dto.enable_control = enable_control;
	dto.max_json_depth = max_json_depth;
	dto.max_request_size = max_request_size;
	dto.rpc_log = rpc_logging.log_rpc;
	dto.rpc_process.io_threads = rpc_process.io_threads;
	std::copy (rpc_process.ipc_address.begin (), rpc_process.ipc_address.end (), std::begin (dto.rpc_process.ipc_address));
	dto.rpc_process.ipc_address_len = rpc_process.ipc_address.size ();
	dto.rpc_process.ipc_port = rpc_process.ipc_port;
	dto.rpc_process.num_ipc_connections = rpc_process.num_ipc_connections;
	return dto;
}

std::string nano::rpc_config::serialize_toml () const
{
	auto dto{ to_dto () };

	const size_t buffer_len = 1000;
	std::vector<char> buffer (buffer_len);

	rsnano::rsn_rpc_config_serialize_toml (&dto, buffer.data (), buffer_len);

	std::string toml_str (buffer.data ());

	return toml_str;
}

nano::rpc_process_config::rpc_process_config (nano::network_constants & network_constants) :
	network_constants{ network_constants },
	ipc_address{ boost::asio::ip::address_v6::loopback ().to_string () }
{
}
