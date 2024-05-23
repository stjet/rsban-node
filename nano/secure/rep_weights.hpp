#pragma once

#include <nano/lib/numbers.hpp>
#include <nano/lib/utility.hpp>

#include <unordered_map>

namespace rsnano
{
class RepWeightsHandle;
}
namespace nano
{
namespace store
{
	class component;
	class write_transaction;
}

class rep_weights
{
public:
	rep_weights (rsnano::RepWeightsHandle * handle_a);
	rep_weights (rep_weights const &) = delete;
	rep_weights (rep_weights &&);
	~rep_weights ();
	rep_weights & operator= (rep_weights && other_a);
	void representation_add (store::write_transaction const & txn_a, nano::account const & source_rep_a, nano::uint128_t const & amount_a);
	void representation_add_dual (store::write_transaction const & txn_a, nano::account const & source_rep_1, nano::uint128_t const & amount_1, nano::account const & source_rep_2, nano::uint128_t const & amount_2);
	nano::uint128_t representation_get (nano::account const & account_a) const;
	std::unordered_map<nano::account, nano::uint128_t> get_rep_amounts () const;

private:
	rsnano::RepWeightsHandle * handle;
};
}
