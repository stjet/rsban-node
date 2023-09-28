#pragma once

#include <nano/store/online_weight.hpp>

namespace nano::store::lmdb
{
class online_weight : public nano::store::online_weight
{
private:
	rsnano::LmdbOnlineWeightStoreHandle * handle;

public:
	explicit online_weight (rsnano::LmdbOnlineWeightStoreHandle * handle_a);
	~online_weight ();
	online_weight (online_weight const &) = delete;
	online_weight (online_weight &&) = delete;
	void put (nano::store::write_transaction const & transaction_a, uint64_t time_a, nano::amount const & amount_a) override;
	void del (nano::store::write_transaction const & transaction_a, uint64_t time_a) override;
	nano::store::iterator<uint64_t, nano::amount> begin (nano::store::transaction const & transaction_a) const override;
	nano::store::iterator<uint64_t, nano::amount> rbegin (nano::store::transaction const & transaction_a) const override;
	nano::store::iterator<uint64_t, nano::amount> end () const override;
	size_t count (nano::store::transaction const & transaction_a) const override;
	void clear (nano::store::write_transaction const & transaction_a) override;
};
}
