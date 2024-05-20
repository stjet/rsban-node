#include <nano/lib/rsnano.hpp>
#include <nano/node/local_vote_history.hpp>
#include <nano/secure/common.hpp>

nano::local_vote_history::local_vote_history (nano::voting_constants const & constants) :
	handle{ rsnano::rsn_local_vote_history_create (constants.max_cache) }
{
}

nano::local_vote_history::local_vote_history (rsnano::LocalVoteHistoryHandle * handle) :
	handle{ handle }
{
}

nano::local_vote_history::~local_vote_history ()
{
	rsnano::rsn_local_vote_history_destroy (handle);
}

void nano::local_vote_history::add (nano::root const & root_a, nano::block_hash const & hash_a, std::shared_ptr<nano::vote> const & vote_a)
{
	rsnano::rsn_local_vote_history_add (handle, root_a.bytes.data (), hash_a.bytes.data (), vote_a->get_handle ());
}

void nano::local_vote_history::erase (nano::root const & root_a)
{
	rsnano::rsn_local_vote_history_erase (handle, root_a.bytes.data ());
}

class LocalVotesResultWrapper
{
public:
	LocalVotesResultWrapper () :
		result{}
	{
	}
	~LocalVotesResultWrapper ()
	{
		rsnano::rsn_local_vote_history_votes_destroy (result.handle);
	}
	rsnano::LocalVotesResult result;
};

std::vector<std::shared_ptr<nano::vote>> nano::local_vote_history::votes (nano::root const & root_a, nano::block_hash const & hash_a, bool const is_final_a) const
{
	LocalVotesResultWrapper result_wrapper;
	rsnano::rsn_local_vote_history_votes (handle, root_a.bytes.data (), hash_a.bytes.data (), is_final_a, &result_wrapper.result);
	std::vector<std::shared_ptr<nano::vote>> votes;
	votes.reserve (result_wrapper.result.count);
	for (auto i (0); i < result_wrapper.result.count; ++i)
	{
		votes.push_back (std::make_shared<nano::vote> (result_wrapper.result.votes[i]));
	}
	return votes;
}

bool nano::local_vote_history::exists (nano::root const & root_a) const
{
	return rsnano::rsn_local_vote_history_exists (handle, root_a.bytes.data ());
}

std::size_t nano::local_vote_history::size () const
{
	return rsnano::rsn_local_vote_history_size (handle);
}

std::unique_ptr<nano::container_info_component> nano::collect_container_info (nano::local_vote_history & history, std::string const & name)
{
	std::size_t sizeof_element;
	std::size_t history_count;
	rsnano::rsn_local_vote_history_container_info (history.handle, &sizeof_element, &history_count);
	auto composite = std::make_unique<container_info_composite> (name);
	/* This does not currently loop over each element inside the cache to get the sizes of the votes inside history*/
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "history", history_count, sizeof_element }));
	return composite;
}
