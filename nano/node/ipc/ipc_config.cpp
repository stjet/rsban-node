#include <nano/lib/jsonconfig.hpp>
#include <nano/lib/tomlconfig.hpp>
#include <nano/node/ipc/ipc_config.hpp>

nano::ipc::ipc_config_tcp_socket::ipc_config_tcp_socket (nano::network_constants network_constants_a)
	: network_constants(network_constants_a)
{
}

nano::ipc::ipc_config::ipc_config (nano::network_constants network_constants)
	: transport_tcp(nano::ipc::ipc_config_tcp_socket(network_constants))
{
	rsnano::IpcConfigDto dto;
	auto network_dto{ network_constants.to_dto () };
	if (rsnano::rsn_ipc_config_create (&dto, &network_dto) < 0)
		throw std::runtime_error ("could not create ipc config");
	load_dto (dto);
}

void nano::ipc::ipc_config::load_dto (rsnano::IpcConfigDto & dto)
{
	transport_domain.enabled = dto.domain_transport.enabled;
	transport_domain.allow_unsafe = dto.domain_transport.allow_unsafe;
	transport_domain.io_timeout = dto.domain_transport.io_timeout;
	transport_domain.io_threads = dto.domain_transport.io_threads;
	transport_domain.path = std::string (reinterpret_cast<const char *> (dto.domain_path), dto.domain_path_len);
	transport_tcp.enabled = dto.tcp_transport.enabled;
	transport_tcp.allow_unsafe = dto.tcp_transport.allow_unsafe;
	transport_tcp.io_timeout = dto.tcp_transport.io_timeout;
	transport_tcp.io_threads = dto.tcp_transport.io_threads;
	transport_tcp.port = dto.tcp_port;
	transport_tcp.network_constants = nano::network_constants (dto.tcp_network_constants);
	flatbuffers.skip_unexpected_fields_in_json = dto.flatbuffers_skip_unexpected_fields_in_json;
	flatbuffers.verify_buffers = dto.flatbuffers_verify_buffers;
}

rsnano::IpcConfigDto nano::ipc::ipc_config::to_dto () const
{
	rsnano::IpcConfigDto dto;
	dto.domain_transport.enabled = transport_domain.enabled;
	dto.domain_transport.allow_unsafe = transport_domain.allow_unsafe;
	dto.domain_transport.io_timeout = transport_domain.io_timeout;
	dto.domain_transport.io_threads = transport_domain.io_threads;
	std::copy (transport_domain.path.begin (), transport_domain.path.end (), std::begin (dto.domain_path));
	dto.domain_path_len = transport_domain.path.size ();
	dto.tcp_transport.enabled = transport_tcp.enabled;
	dto.tcp_transport.allow_unsafe = transport_tcp.allow_unsafe;
	dto.tcp_transport.io_timeout = transport_tcp.io_timeout;
	dto.tcp_transport.io_threads = transport_tcp.io_threads;
	dto.tcp_port = transport_tcp.port;
	dto.tcp_network_constants = transport_tcp.network_constants.to_dto ();
	dto.flatbuffers_skip_unexpected_fields_in_json = flatbuffers.skip_unexpected_fields_in_json;
	dto.flatbuffers_verify_buffers = flatbuffers.verify_buffers;
	return dto;
}

nano::error nano::ipc::ipc_config::deserialize_toml (nano::tomlconfig & toml)
{
	auto tcp_l (toml.get_optional_child ("tcp"));
	if (tcp_l)
	{
		tcp_l->get_optional<long> ("io_threads", transport_tcp.io_threads, -1);
		tcp_l->get<bool> ("allow_unsafe", transport_tcp.allow_unsafe);
		tcp_l->get<bool> ("enable", transport_tcp.enabled);
		tcp_l->get<uint16_t> ("port", transport_tcp.port);
		tcp_l->get<std::size_t> ("io_timeout", transport_tcp.io_timeout);
	}

	auto domain_l (toml.get_optional_child ("local"));
	if (domain_l)
	{
		domain_l->get_optional<long> ("io_threads", transport_domain.io_threads, -1);
		domain_l->get<bool> ("allow_unsafe", transport_domain.allow_unsafe);
		domain_l->get<bool> ("enable", transport_domain.enabled);
		domain_l->get<std::string> ("path", transport_domain.path);
		domain_l->get<std::size_t> ("io_timeout", transport_domain.io_timeout);
	}

	auto flatbuffers_l (toml.get_optional_child ("flatbuffers"));
	if (flatbuffers_l)
	{
		flatbuffers_l->get<bool> ("skip_unexpected_fields_in_json", flatbuffers.skip_unexpected_fields_in_json);
		flatbuffers_l->get<bool> ("verify_buffers", flatbuffers.verify_buffers);
	}

	return toml.get_error ();
}
