#include <nano/lib/config.hpp>
#include <nano/lib/jsonconfig.hpp>
#include <nano/lib/tomlconfig.hpp>
#include <nano/node/daemonconfig.hpp>

#include <sstream>
#include <vector>

nano::daemon_config::daemon_config (std::filesystem::path const & data_path_a, nano::network_params & network_params) :
	node{ network_params },
	data_path{ data_path_a }
{
	rsnano::DaemonConfigDto dto;
	auto network_dto{ network_params.to_dto () };
	if (rsnano::rsn_daemon_config_create (&dto, &network_dto) < 0)
		throw std::runtime_error ("could not create daemon_config");
	rpc_enable = dto.rpc_enable;
	node.load_dto (dto.node);
	opencl.load_dto (dto.opencl);
	opencl_enable = dto.opencl_enable;
	rpc.load_dto (dto.rpc);
}

rsnano::DaemonConfigDto to_daemon_config_dto (nano::daemon_config const & config)
{
	rsnano::DaemonConfigDto dto;
	dto.rpc_enable = config.rpc_enable;
	dto.opencl = config.opencl.to_dto ();
	dto.node = config.node.to_dto ();
	dto.opencl_enable = config.opencl_enable;
	dto.rpc = config.rpc.to_dto ();
	return dto;
}

std::string nano::daemon_config::serialize_toml ()
{
	auto dto{ to_daemon_config_dto (*this) };

	const size_t buffer_len = 10000;
	std::vector<char> buffer (buffer_len);

	rsnano::rsn_daemon_config_serialize_toml (&dto, buffer.data (), buffer_len);

	std::string toml_str (buffer.data ());

	return toml_str;
}
