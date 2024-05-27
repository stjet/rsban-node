#pragma once

#include <nano/store/confirmation_height.hpp>

namespace nano::store::lmdb
{
class store;
class confirmation_height : public nano::store::confirmation_height
{
	rsnano::LmdbConfirmationHeightStoreHandle * handle;

public:
	explicit confirmation_height (rsnano::LmdbConfirmationHeightStoreHandle * handle_a);
	~confirmation_height ();
	confirmation_height (confirmation_height const &) = delete;
	confirmation_height (confirmation_height &&) = delete;
	void put (nano::store::write_transaction const & transaction_a, nano::account const & account_a, nano::confirmation_height_info const & confirmation_height_info_a) override;
	bool get (nano::store::transaction const & transaction_a, nano::account const & account_a, nano::confirmation_height_info & confirmation_height_info_a) override;
	bool exists (nano::store::transaction const & transaction_a, nano::account const & account_a) const override;
	void del (nano::store::write_transaction const & transaction_a, nano::account const & account_a) override;
	uint64_t count (nano::store::transaction const & transaction_a) override;
	void clear (nano::store::write_transaction const & transaction_a, nano::account const & account_a) override;
	void clear (nano::store::write_transaction const & transaction_a) override;
	nano::store::iterator<nano::account, nano::confirmation_height_info> begin (nano::store::transaction const & transaction_a, nano::account const & account_a) const override;
	nano::store::iterator<nano::account, nano::confirmation_height_info> begin (nano::store::transaction const & transaction_a) const override;
	nano::store::iterator<nano::account, nano::confirmation_height_info> end () const override;
};
}
