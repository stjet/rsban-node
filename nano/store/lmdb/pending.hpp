#pragma once

#include <nano/store/pending.hpp>

namespace nano::store::lmdb
{
class pending : public nano::store::pending
{
private:
	rsnano::LmdbPendingStoreHandle * handle;

public:
	explicit pending (rsnano::LmdbPendingStoreHandle * handle_a);
	~pending ();
	pending (pending const &) = delete;
	pending (pending &&) = delete;
	void put (nano::store::write_transaction const & transaction_a, nano::pending_key const & key_a, nano::pending_info const & pending_info_a) override;
	void del (nano::store::write_transaction const & transaction_a, nano::pending_key const & key_a) override;
	std::optional<nano::pending_info> get (nano::store::transaction const & transaction_a, nano::pending_key const &) override;
	bool exists (nano::store::transaction const & transaction_a, nano::pending_key const & key_a) override;
	bool any (nano::store::transaction const & transaction_a, nano::account const & account_a) override;
	nano::store::iterator<nano::pending_key, nano::pending_info> begin (nano::store::transaction const & transaction_a, nano::pending_key const & key_a) const override;
	nano::store::iterator<nano::pending_key, nano::pending_info> begin (nano::store::transaction const & transaction_a) const override;
	nano::store::iterator<nano::pending_key, nano::pending_info> end () const override;
};
}
