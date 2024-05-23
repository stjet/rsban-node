#include <nano/lib/rsnano.hpp>
#include <nano/node/online_reps.hpp>
#include <nano/node/rep_tiers.hpp>
#include <nano/secure/common.hpp>
#include <nano/secure/ledger.hpp>

using namespace std::chrono_literals;

nano::rep_tiers::rep_tiers (nano::ledger & ledger_a, nano::network_params & network_params_a, nano::online_reps & online_reps_a, nano::stats & stats_a)
{
	auto network_params_dto{ network_params_a.to_dto () };
	handle = rsnano::rsn_rep_tiers_create (ledger_a.handle, &network_params_dto, online_reps_a.get_handle (), stats_a.handle);
}

nano::rep_tiers::rep_tiers (rsnano::RepTiersHandle * handle) :
	handle{ handle }
{
}

nano::rep_tiers::~rep_tiers ()
{
	rsnano::rsn_rep_tiers_destroy (handle);
}

void nano::rep_tiers::start ()
{
	rsnano::rsn_rep_tiers_start (handle);
}

void nano::rep_tiers::stop ()
{
	rsnano::rsn_rep_tiers_stop (handle);
}

nano::rep_tier nano::rep_tiers::tier (const nano::account & representative) const
{
	return static_cast<nano::rep_tier> (rsnano::rsn_rep_tiers_tier (handle, representative.bytes.data ()));
}
