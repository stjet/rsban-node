#pragma once
#include <nano/store/frontier.hpp>

namespace nano::store::lmdb
{
class frontier : public nano::store::frontier
{
private:
	rsnano::LmdbFrontierStoreHandle * handle;

public:
	explicit frontier (rsnano::LmdbFrontierStoreHandle * handle_a);
	~frontier ();
	frontier (frontier const &) = delete;
	frontier (frontier &&) = delete;
	void put (nano::store::write_transaction const &, nano::block_hash const &, nano::account const &) override;
	nano::account get (nano::store::transaction const &, nano::block_hash const &) const override;
	void del (nano::store::write_transaction const &, nano::block_hash const &) override;
	nano::store::iterator<nano::block_hash, nano::account> begin (nano::store::transaction const &) const override;
	nano::store::iterator<nano::block_hash, nano::account> begin (nano::store::transaction const &, nano::block_hash const &) const override;
	nano::store::iterator<nano::block_hash, nano::account> end () const override;
	void for_each_par (std::function<void (nano::store::read_transaction const &, nano::store::iterator<nano::block_hash, nano::account>, nano::store::iterator<nano::block_hash, nano::account>)> const & action_a) const override;
};
}
