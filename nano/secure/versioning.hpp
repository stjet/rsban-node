#pragma once

#include <nano/lib/blocks.hpp>
#include <nano/secure/common.hpp>

struct MDB_val;

namespace nano
{
class block_sideband_v18 final
{
public:
	block_sideband_v18 () = default;
	block_sideband_v18 (nano::account const &, nano::block_hash const &, nano::amount const &, uint64_t, uint64_t, nano::block_details const &);
	block_sideband_v18 (nano::account const &, nano::block_hash const &, nano::amount const &, uint64_t, uint64_t, nano::epoch, bool is_send, bool is_receive, bool is_epoch);
	void serialize (nano::stream &, nano::block_type) const;
	bool deserialize (nano::stream &, nano::block_type);
	static size_t size (nano::block_type);
	nano::block_hash successor{ 0 };
	nano::account account{};
	nano::amount balance{ 0 };
	uint64_t height{ 0 };
	uint64_t timestamp{ 0 };
	nano::block_details details;
};
}
