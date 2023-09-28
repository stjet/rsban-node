#include <nano/store/lmdb/account.hpp>
#include <nano/store/lmdb/db_val.hpp>
#include <nano/store/lmdb/lmdb.hpp>

nano::store::lmdb::account::account (rsnano::LmdbAccountStoreHandle * handle_a) :
	handle{ handle_a }
{
}

nano::store::lmdb::account::~account ()
{
	if (handle != nullptr)
		rsnano::rsn_lmdb_account_store_destroy (handle);
}

void nano::store::lmdb::account::put (nano::store::write_transaction const & transaction, nano::account const & account, nano::account_info const & info)
{
	rsnano::rsn_lmdb_account_store_put (handle, transaction.get_rust_handle (), account.bytes.data (), info.handle);
}

bool nano::store::lmdb::account::get (nano::store::transaction const & transaction, nano::account const & account, nano::account_info & info)
{
	bool found = rsnano::rsn_lmdb_account_store_get (handle, transaction.get_rust_handle (), account.bytes.data (), info.handle);
	return !found;
}

void nano::store::lmdb::account::del (nano::store::write_transaction const & transaction_a, nano::account const & account_a)
{
	rsnano::rsn_lmdb_account_store_del (handle, transaction_a.get_rust_handle (), account_a.bytes.data ());
}

bool nano::store::lmdb::account::exists (nano::store::transaction const & transaction_a, nano::account const & account_a)
{
	auto iterator (begin (transaction_a, account_a));
	return iterator != end () && nano::account (iterator->first) == account_a;
}

size_t nano::store::lmdb::account::count (nano::store::transaction const & transaction_a)
{
	return rsnano::rsn_lmdb_account_store_count (handle, transaction_a.get_rust_handle ());
}

nano::store::iterator<nano::account, nano::account_info> to_account_iterator (rsnano::LmdbIteratorHandle * it_handle)
{
	if (it_handle == nullptr)
	{
		return nano::store::iterator<nano::account, nano::account_info> (nullptr);
	}

	return nano::store::iterator<nano::account, nano::account_info> (
	std::make_unique<nano::store::lmdb::iterator<nano::account, nano::account_info>> (it_handle));
}

nano::store::iterator<nano::account, nano::account_info> nano::store::lmdb::account::begin (nano::store::transaction const & transaction, nano::account const & account) const
{
	auto it_handle{ rsnano::rsn_lmdb_account_store_begin_account (handle, transaction.get_rust_handle (), account.bytes.data ()) };
	return to_account_iterator (it_handle);
}

nano::store::iterator<nano::account, nano::account_info> nano::store::lmdb::account::begin (nano::store::transaction const & transaction) const
{
	auto it_handle{ rsnano::rsn_lmdb_account_store_begin (handle, transaction.get_rust_handle ()) };
	return to_account_iterator (it_handle);
}

nano::store::iterator<nano::account, nano::account_info> nano::store::lmdb::account::end () const
{
	return nano::store::iterator<nano::account, nano::account_info> (nullptr);
}

namespace
{
void for_each_par_wrapper (void * context, rsnano::TransactionHandle * txn_handle, rsnano::LmdbIteratorHandle * begin_handle, rsnano::LmdbIteratorHandle * end_handle)
{
	auto action = static_cast<std::function<void (nano::store::read_transaction const &, nano::store::iterator<nano::account, nano::account_info>, nano::store::iterator<nano::account, nano::account_info>)> const *> (context);
	nano::store::lmdb::read_transaction_impl txn{ txn_handle };
	auto begin{ to_account_iterator (begin_handle) };
	auto end{ to_account_iterator (end_handle) };
	(*action) (txn, std::move (begin), std::move (end));
}
void for_each_par_delete_context (void * context)
{
}
}

void nano::store::lmdb::account::for_each_par (std::function<void (nano::store::read_transaction const &, nano::store::iterator<nano::account, nano::account_info>, nano::store::iterator<nano::account, nano::account_info>)> const & action_a) const
{
	auto context = (void *)&action_a;
	rsnano::rsn_lmdb_account_store_for_each_par (handle, for_each_par_wrapper, context, for_each_par_delete_context);
}
