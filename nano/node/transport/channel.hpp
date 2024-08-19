#pragma once

#include <nano/lib/locks.hpp>
#include <nano/lib/stats.hpp>
#include <nano/node/common.hpp>
#include <nano/node/messages.hpp>
#include <boost/asio/ip/network_v6.hpp>
#include <cstdint>

namespace nano::transport
{
enum class transport_type : uint8_t
{
	undefined = 0,
	tcp = 1,
};
}
