#include "nano/lib/rsnano.hpp"

#include <nano/lib/blocks.hpp>
#include <nano/node/confirmation_solicitor.hpp>
#include <nano/node/election.hpp>
#include <nano/node/nodeconfig.hpp>

using namespace std::chrono_literals;

nano::confirmation_solicitor::confirmation_solicitor (nano::network & network_a, nano::node_config const & config_a)
{
	auto params_dto{ config_a.network_params.to_dto () };
	handle = rsnano::rsn_confirmation_solicitor_create (&params_dto, network_a.tcp_channels->handle);
}

nano::confirmation_solicitor::~confirmation_solicitor ()
{
	rsnano::rsn_confirmation_solicitor_destroy (handle);
}

void nano::confirmation_solicitor::prepare (std::vector<nano::representative> const & representatives_a)
{
	auto reps_handle = rsnano::rsn_representative_vec_create ();
	for (const auto & i : representatives_a)
	{
		rsnano::rsn_representative_vec_push (reps_handle, i.handle);
	}
	rsnano::rsn_confirmation_solicitor_prepare (handle, reps_handle);
	rsnano::rsn_representative_vec_destroy (reps_handle);
}

bool nano::confirmation_solicitor::broadcast (nano::election const & election_a, nano::election_lock const & lock_a)
{
	return rsnano::rsn_confirmation_solicitor_broadcast (handle, lock_a.handle);
}

bool nano::confirmation_solicitor::add (nano::election const & election_a, nano::election_lock const & lock_a)
{
	return rsnano::rsn_confirmation_solicitor_add (handle, election_a.handle, lock_a.handle);
}

void nano::confirmation_solicitor::flush ()
{
	rsnano::rsn_confirmation_solicitor_flush (handle);
}
