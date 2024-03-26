#include <nano/lib/rsnano.hpp>
#include <nano/node/vote_spacing.hpp>

nano::vote_spacing::vote_spacing (std::chrono::milliseconds const & delay) :
	handle{ rsnano::rsn_vote_spacing_create (delay.count ()) }
{
}

nano::vote_spacing::~vote_spacing ()
{
	rsnano::rsn_vote_spacing_destroy (handle);
}

bool nano::vote_spacing::votable (nano::root const & root_a, nano::block_hash const & hash_a) const
{
	return rsnano::rsn_vote_spacing_votable (handle, root_a.bytes.data (), hash_a.bytes.data ());
}

void nano::vote_spacing::flag (nano::root const & root_a, nano::block_hash const & hash_a)
{
	rsnano::rsn_vote_spacing_flag (handle, root_a.bytes.data (), hash_a.bytes.data ());
}

std::size_t nano::vote_spacing::size () const
{
	return rsnano::rsn_vote_spacing_len (handle);
}
