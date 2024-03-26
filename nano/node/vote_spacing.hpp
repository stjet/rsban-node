#pragma once

#include <nano/lib/numbers.hpp>
#include <chrono>

namespace rsnano
{
class VoteSpacingHandle;
}

namespace nano
{
class local_vote_history;

class vote_spacing final
{
public:
	vote_spacing (std::chrono::milliseconds const & delay);
	vote_spacing (vote_spacing const &) = delete;
	vote_spacing (vote_spacing &&) = delete;
	~vote_spacing ();
	bool votable (nano::root const & root_a, nano::block_hash const & hash_a) const;
	void flag (nano::root const & root_a, nano::block_hash const & hash_a);
	std::size_t size () const;
	rsnano::VoteSpacingHandle * handle;
};
}
