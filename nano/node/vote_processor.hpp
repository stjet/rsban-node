#pragma once

#include "nano/lib/rsnano.hpp"

#include <nano/lib/numbers.hpp>
#include <nano/lib/utility.hpp>
#include <nano/secure/common.hpp>

#include <memory>

namespace nano
{

namespace transport
{
	class channel;
}

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

class vote_processor_queue
{
public:
	vote_processor_queue (rsnano::VoteProcessorQueueHandle * handle);
	vote_processor_queue (vote_processor_queue const &) = delete;
	~vote_processor_queue ();

	rsnano::VoteProcessorQueueHandle * handle;
};

class vote_processor final
{
public:
	vote_processor (rsnano::VoteProcessorHandle * handle);
	vote_processor (vote_processor const &) = delete;
	~vote_processor ();

	/** Note: node.active.mutex lock is required */
	nano::vote_code vote_blocking (std::shared_ptr<nano::vote> const &, std::shared_ptr<nano::transport::channel> const &);

	rsnano::VoteProcessorHandle * handle;
};

}
