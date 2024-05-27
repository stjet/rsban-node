#include <nano/store/lmdb/confirmation_height.hpp>
#include <nano/store/lmdb/lmdb.hpp>

namespace
{
nano::store::iterator<nano::account, nano::confirmation_height_info> to_iterator (rsnano::LmdbIteratorHandle * it_handle)
{
	if (it_handle == nullptr)
	{
		return nano::store::iterator<nano::account, nano::confirmation_height_info> (nullptr);
	}

	return nano::store::iterator<nano::account, nano::confirmation_height_info> (
	std::make_unique<nano::store::lmdb::iterator<nano::account, nano::confirmation_height_info>> (it_handle));
}
}

nano::store::lmdb::confirmation_height::confirmation_height (rsnano::LmdbConfirmationHeightStoreHandle * handle_a) :
	handle{ handle_a }
{
}

nano::store::lmdb::confirmation_height::~confirmation_height ()
{
	if (handle != nullptr)
		rsnano::rsn_lmdb_confirmation_height_store_destroy (handle);
}

void nano::store::lmdb::confirmation_height::put (nano::store::write_transaction const & transaction, nano::account const & account, nano::confirmation_height_info const & confirmation_height_info)
{
	rsnano::rsn_lmdb_confirmation_height_store_put (handle, transaction.get_rust_handle (), account.bytes.data (), &confirmation_height_info.dto);
}

bool nano::store::lmdb::confirmation_height::get (nano::store::transaction const & transaction, nano::account const & account, nano::confirmation_height_info & confirmation_height_info)
{
	bool success = rsnano::rsn_lmdb_confirmation_height_store_get (handle, transaction.get_rust_handle (), account.bytes.data (), &confirmation_height_info.dto);
	return !success;
}

bool nano::store::lmdb::confirmation_height::exists (nano::store::transaction const & transaction, nano::account const & account) const
{
	return rsnano::rsn_lmdb_confirmation_height_store_exists (handle, transaction.get_rust_handle (), account.bytes.data ());
}

void nano::store::lmdb::confirmation_height::del (nano::store::write_transaction const & transaction, nano::account const & account)
{
	rsnano::rsn_lmdb_confirmation_height_store_del (handle, transaction.get_rust_handle (), account.bytes.data ());
}

uint64_t nano::store::lmdb::confirmation_height::count (nano::store::transaction const & transaction_a)
{
	return rsnano::rsn_lmdb_confirmation_height_store_count (handle, transaction_a.get_rust_handle ());
}

void nano::store::lmdb::confirmation_height::clear (nano::store::write_transaction const & transaction_a, nano::account const & account_a)
{
	del (transaction_a, account_a);
}

void nano::store::lmdb::confirmation_height::clear (nano::store::write_transaction const & transaction_a)
{
	rsnano::rsn_lmdb_confirmation_height_store_clear (handle, transaction_a.get_rust_handle ());
}

nano::store::iterator<nano::account, nano::confirmation_height_info> nano::store::lmdb::confirmation_height::begin (nano::store::transaction const & transaction, nano::account const & account) const
{
	auto it_handle{ rsnano::rsn_lmdb_confirmation_height_store_begin_at_account (handle, transaction.get_rust_handle (), account.bytes.data ()) };
	return to_iterator (it_handle);
}

nano::store::iterator<nano::account, nano::confirmation_height_info> nano::store::lmdb::confirmation_height::begin (nano::store::transaction const & transaction) const
{
	auto it_handle{ rsnano::rsn_lmdb_confirmation_height_store_begin (handle, transaction.get_rust_handle ()) };
	return to_iterator (it_handle);
}

nano::store::iterator<nano::account, nano::confirmation_height_info> nano::store::lmdb::confirmation_height::end () const
{
	return nano::store::iterator<nano::account, nano::confirmation_height_info> (nullptr);
}

