#pragma once

#include <nano/store/pruned.hpp>

namespace nano::store::lmdb
{
class pruned : public nano::store::pruned
{
private:
	rsnano::LmdbPrunedStoreHandle * handle;

public:
	explicit pruned (rsnano::LmdbPrunedStoreHandle * handle_a);
	~pruned ();
	pruned (pruned const &) = delete;
	pruned (pruned &&) = delete;
	void put (nano::store::write_transaction const & transaction_a, nano::block_hash const & hash_a) override;
	void del (nano::store::write_transaction const & transaction_a, nano::block_hash const & hash_a) override;
	bool exists (nano::store::transaction const & transaction_a, nano::block_hash const & hash_a) const override;
	nano::block_hash random (nano::store::transaction const & transaction_a) override;
	size_t count (nano::store::transaction const & transaction_a) const override;
	void clear (nano::store::write_transaction const & transaction_a) override;
	nano::store::iterator<nano::block_hash, std::nullptr_t> begin (nano::store::transaction const & transaction_a, nano::block_hash const & hash_a) const override;
	nano::store::iterator<nano::block_hash, std::nullptr_t> begin (nano::store::transaction const & transaction_a) const override;
	nano::store::iterator<nano::block_hash, std::nullptr_t> end () const override;
	void for_each_par (std::function<void (nano::store::read_transaction const &, nano::store::iterator<nano::block_hash, std::nullptr_t>, nano::store::iterator<nano::block_hash, std::nullptr_t>)> const & action_a) const override;
};
}
