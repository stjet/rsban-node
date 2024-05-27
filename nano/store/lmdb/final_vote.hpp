#pragma once

#include <nano/store/final.hpp>

namespace nano::store::lmdb
{
class final_vote : public nano::store::final_vote
{
public:
	explicit final_vote (rsnano::LmdbFinalVoteStoreHandle * handle_a);
	~final_vote ();
	final_vote (final_vote const &) = delete;
	final_vote (final_vote &&) = delete;
	bool put (nano::store::write_transaction const & transaction_a, nano::qualified_root const & root_a, nano::block_hash const & hash_a) override;
	std::vector<nano::block_hash> get (nano::store::transaction const & transaction_a, nano::root const & root_a) override;
	void del (nano::store::write_transaction const & transaction_a, nano::root const & root_a) override;
	size_t count (nano::store::transaction const & transaction_a) const override;
	void clear (nano::store::write_transaction const & transaction_a, nano::root const & root_a) override;
	void clear (nano::store::write_transaction const & transaction_a) override;
	nano::store::iterator<nano::qualified_root, nano::block_hash> begin (nano::store::transaction const & transaction_a, nano::qualified_root const & root_a) const override;
	nano::store::iterator<nano::qualified_root, nano::block_hash> begin (nano::store::transaction const & transaction_a) const override;
	nano::store::iterator<nano::qualified_root, nano::block_hash> end () const override;
	rsnano::LmdbFinalVoteStoreHandle * handle;
};
}
