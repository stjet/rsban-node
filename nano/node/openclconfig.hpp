#pragma once

#include <nano/lib/errors.hpp>
#include <nano/lib/rsnano.hpp>

namespace nano
{
class jsonconfig;
class tomlconfig;
class opencl_config
{
public:
	opencl_config () = default;
	opencl_config (unsigned, unsigned, unsigned);
	void load_dto (rsnano::OpenclConfigDto & dto);
	rsnano::OpenclConfigDto to_dto () const;
	nano::error serialize_json (nano::jsonconfig &) const;
	nano::error deserialize_json (nano::jsonconfig &);
	nano::error deserialize_toml (nano::tomlconfig &);
	unsigned platform{ 0 };
	unsigned device{ 0 };
	unsigned threads{ 1024 * 1024 };
};
}
