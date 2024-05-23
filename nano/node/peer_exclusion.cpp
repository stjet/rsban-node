#include "nano/lib/rsnanoutils.hpp"

#include <nano/node/common.hpp>
#include <nano/node/peer_exclusion.hpp>

nano::peer_exclusion::peer_exclusion (std::size_t max_size_a) :
	handle{ rsnano::rsn_peer_exclusion_create (max_size_a) }
{
}

nano::peer_exclusion::peer_exclusion (rsnano::PeerExclusionHandle * handle_a) :
	handle{ handle_a }
{
}

nano::peer_exclusion::~peer_exclusion ()
{
	rsnano::rsn_peer_exclusion_destroy (handle);
}

uint64_t nano::peer_exclusion::add (nano::tcp_endpoint const & endpoint_a)
{
	auto endpoint_dto{ rsnano::endpoint_to_dto (endpoint_a) };
	return rsnano::rsn_peer_exclusion_add (handle, &endpoint_dto);
}

bool nano::peer_exclusion::check (nano::tcp_endpoint const & endpoint) const
{
	auto endpoint_dto{ rsnano::endpoint_to_dto (endpoint) };
	return rsnano::rsn_peer_exclusion_check (handle, &endpoint_dto);
}

uint64_t nano::peer_exclusion::score (const nano::tcp_endpoint & endpoint) const
{
	auto endpoint_dto{ rsnano::endpoint_to_dto (endpoint) };
	return rsnano::rsn_peer_exclusion_score (handle, &endpoint_dto);
}

void nano::peer_exclusion::remove (nano::tcp_endpoint const & endpoint_a)
{
	auto endpoint_dto{ rsnano::endpoint_to_dto (endpoint_a) };
	rsnano::rsn_peer_exclusion_remove (handle, &endpoint_dto);
}

std::size_t nano::peer_exclusion::size () const
{
	return rsnano::rsn_peer_exclusion_size (handle);
}

bool nano::peer_exclusion::contains (const nano::tcp_endpoint & endpoint_a)
{
	auto endpoint_dto{ rsnano::endpoint_to_dto (endpoint_a) };
	return rsnano::rsn_peer_exclusion_contains (handle, &endpoint_dto);
}
