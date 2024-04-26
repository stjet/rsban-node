#pragma once

#include <nano/store/account.hpp>

namespace nano::store::lmdb
{
class component;
}
namespace nano::store::lmdb
{
class account : public nano::store::account
{
public:
	explicit account (rsnano::LmdbAccountStoreHandle * handle_a);
	account (account const &) = delete;
	account (account &&) = delete;
	~account () override;
	void put (nano::store::write_transaction const & transaction, nano::account const & account, nano::account_info const & info) override;
	bool get (nano::store::transaction const & transaction_a, nano::account const & account_a, nano::account_info & info_a) const override;
	std::optional<nano::account_info> get (nano::store::transaction const &, nano::account const &) override;
	void del (nano::store::write_transaction const & transaction_a, nano::account const & account_a) override;
	bool exists (nano::store::transaction const & transaction_a, nano::account const & account_a) override;
	size_t count (nano::store::transaction const & transaction_a) override;
	nano::store::iterator<nano::account, nano::account_info> begin (nano::store::transaction const & transaction_a, nano::account const & account_a) const override;
	nano::store::iterator<nano::account, nano::account_info> begin (nano::store::transaction const & transaction_a) const override;
	nano::store::iterator<nano::account, nano::account_info> end () const override;

private:
	rsnano::LmdbAccountStoreHandle * handle;
};
}
