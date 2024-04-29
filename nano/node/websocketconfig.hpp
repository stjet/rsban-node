#pragma once

#include <nano/lib/config.hpp>
#include <nano/lib/errors.hpp>

#include <memory>

namespace nano
{
class tomlconfig;
namespace websocket
{
	/** websocket configuration */
	class config final
	{
	public:
		config (nano::network_constants & network_constants);
		void load_dto (rsnano::WebsocketConfigDto & dto);
		rsnano::WebsocketConfigDto to_dto () const;
		nano::error deserialize_toml (nano::tomlconfig & toml_a);
		nano::network_constants & network_constants;
		bool enabled;
		uint16_t port;
		std::string address;
		/** Optional TLS config */
	};
}
}
