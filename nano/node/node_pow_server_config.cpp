#include <nano/lib/tomlconfig.hpp>
#include <nano/node/node_pow_server_config.hpp>

rsnano::NodePowServerConfigDto nano::node_pow_server_config::to_dto () const
{
	rsnano::NodePowServerConfigDto dto;
	dto.enable = enable;
	std::copy (pow_server_path.begin (), pow_server_path.end (), std::begin (dto.pow_server_path));
	dto.pow_server_path_len = pow_server_path.size ();
	return dto;
}

void nano::node_pow_server_config::load_dto (rsnano::NodePowServerConfigDto & dto)
{
	enable = dto.enable;
	pow_server_path = std::string (reinterpret_cast<const char *> (dto.pow_server_path), dto.pow_server_path_len);
}

nano::error nano::node_pow_server_config::deserialize_toml (nano::tomlconfig & toml)
{
	toml.get_optional<bool> ("enable", enable);
	toml.get_optional<std::string> ("nano_pow_server_path", pow_server_path);

	return toml.get_error ();
}
