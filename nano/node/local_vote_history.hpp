#pragma once

#include <nano/lib/numbers.hpp>

namespace rsnano
{
class LocalVoteHistoryHandle;
}

namespace nano
{
class container_info_component;
class vote;
class voting_constants;
}

namespace nano
{
class local_vote_history final
{
public:
	local_vote_history (rsnano::LocalVoteHistoryHandle * handle);
	local_vote_history (const local_vote_history &) = delete;
	local_vote_history (local_vote_history &&) = delete;
	~local_vote_history ();
	void add (nano::root const & root_a, nano::block_hash const & hash_a, std::shared_ptr<nano::vote> const & vote_a);
	void erase (nano::root const & root_a);

	std::vector<std::shared_ptr<nano::vote>> votes (nano::root const & root_a, nano::block_hash const & hash_a, bool const is_final_a = false) const;

	rsnano::LocalVoteHistoryHandle * handle;

private:
	friend class local_vote_history_basic_Test;
};
}
