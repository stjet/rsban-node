#include <nano/lib/jsonconfig.hpp>
#include <nano/lib/tomlconfig.hpp>
#include <nano/node/openclconfig.hpp>

nano::opencl_config::opencl_config (unsigned platform_a, unsigned device_a, unsigned threads_a) :
	platform (platform_a),
	device (device_a),
	threads (threads_a)
{
}

void nano::opencl_config::load_dto (rsnano::OpenclConfigDto & dto)
{
	platform = dto.platform;
	device = dto.device;
	threads = dto.threads;
}

rsnano::OpenclConfigDto nano::opencl_config::to_dto () const
{
	rsnano::OpenclConfigDto dto;
	dto.platform = platform;
	dto.device = device;
	dto.threads = threads;
	return dto;
}

nano::error nano::opencl_config::serialize_json (nano::jsonconfig & json) const
{
	json.put ("platform", platform);
	json.put ("device", device);
	json.put ("threads", threads);
	return json.get_error ();
}

nano::error nano::opencl_config::deserialize_json (nano::jsonconfig & json)
{
	json.get_optional<unsigned> ("platform", platform);
	json.get_optional<unsigned> ("device", device);
	json.get_optional<unsigned> ("threads", threads);
	return json.get_error ();
}

nano::error nano::opencl_config::deserialize_toml (nano::tomlconfig & toml)
{
	toml.get_optional<unsigned> ("platform", platform);
	toml.get_optional<unsigned> ("device", device);
	toml.get_optional<unsigned> ("threads", threads);
	return toml.get_error ();
}
