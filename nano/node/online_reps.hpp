#pragma once

#include "nano/lib/rsnano.hpp"

#include <nano/lib/numbers.hpp>
#include <nano/lib/utility.hpp>
#include <nano/secure/common.hpp>

#include <vector>

namespace nano
{
class ledger;
class node_config;

/** Track online representatives and trend online weight */
class online_reps final
{
public:
	online_reps (rsnano::OnlineRepsHandle * handle);
	online_reps (online_reps const &) = delete;
	online_reps (online_reps &&) = delete;
	~online_reps ();
	/** List of online representatives, both the currently sampling ones and the ones observed in the previous sampling period */
	std::vector<nano::account> list ();
	void set_online (nano::uint128_t);
	rsnano::OnlineRepsHandle * get_handle () const;

private:
	rsnano::OnlineRepsHandle * handle;

	friend class election_quorum_minimum_update_weight_before_quorum_checks_Test;
};

uint8_t online_weight_quorum ();
}
