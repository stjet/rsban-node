#pragma once

#include <nano/lib/numbers.hpp>
#include <nano/lib/rsnano.hpp>
#include <nano/lib/rsnanoutils.hpp>

#include <chrono>

namespace nano
{
class vote_with_weight_info final
{
public:
	vote_with_weight_info () = default;

	vote_with_weight_info (
	nano::account representative,
	std::chrono::system_clock::time_point time,
	uint64_t timestamp,
	nano::block_hash hash,
	nano::uint128_t weight) :
		representative{ representative },
		time{ time },
		timestamp{ timestamp },
		hash{ hash },
		weight{ weight }
	{
	}

	explicit vote_with_weight_info (rsnano::VoteWithWeightInfoDto const & dto) :
		representative{ nano::account::from_bytes (dto.representative) },
		time{ rsnano::time_point_from_nanoseconds (dto.time_ns) },
		timestamp{ dto.timestamp },
		hash{ nano::block_hash::from_bytes (dto.hash) },
		weight{ nano::amount::from_bytes (dto.weight).number () }
	{
	}

	nano::account representative;
	std::chrono::system_clock::time_point time;
	uint64_t timestamp;
	nano::block_hash hash;
	nano::uint128_t weight;
};
}
