#pragma once

#include "nano/lib/rsnano.hpp"

#include <nano/lib/numbers.hpp>
#include <nano/lib/utility.hpp>
#include <nano/secure/common.hpp>

namespace nano
{
class vote_processor_config final
{
public:
	vote_processor_config () = default;
	vote_processor_config (rsnano::VoteProcessorConfigDto const & dto);
	nano::error deserialize (nano::tomlconfig & toml);
	rsnano::VoteProcessorConfigDto to_dto () const;

public:
	size_t max_pr_queue{ 256 };
	size_t max_non_pr_queue{ 32 };
	size_t pr_priority{ 3 };
	size_t threads{ 4 };
	size_t batch_size{ 1024 };
	size_t max_triggered{ 16384 };
};
}
