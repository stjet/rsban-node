#include <nano/node/lmdb/account_store.hpp>
#include <nano/node/lmdb/lmdb.hpp>
#include <nano/secure/parallel_traversal.hpp>

nano::lmdb::account_store::account_store (nano::lmdb::store & store_a) :
	store (store_a){};

bool nano::lmdb::account_store::open_databases (nano::transaction const & transaction_a, unsigned flags)
{
	bool error = mdb_dbi_open (to_mdb_txn (transaction_a), "accounts", flags, &accounts_v0_handle) != MDB_SUCCESS;
	accounts_handle = accounts_v0_handle;
	return error;
}

void nano::lmdb::account_store::put (nano::write_transaction const & transaction, nano::account const & account, nano::account_info const & info)
{
	nano::mdb_val account_val{ account };
	nano::mdb_val info_val{ info };
	auto status = mdb_put (to_mdb_txn (transaction), accounts_handle, account_val, info_val, 0);
	nano::assert_success (status);
}

bool nano::lmdb::account_store::get (nano::transaction const & transaction, nano::account const & account, nano::account_info & info)
{
	nano::mdb_val key{ account };
	nano::mdb_val value;
	auto status1 = mdb_get (to_mdb_txn (transaction), accounts_handle, key, value);

	release_assert (status1 == MDB_SUCCESS || status1 == MDB_NOTFOUND);
	bool result (true);
	if (status1 == MDB_SUCCESS)
	{
		nano::bufferstream stream (reinterpret_cast<uint8_t const *> (value.data ()), value.size ());
		result = info.deserialize (stream);
	}
	return result;
}

void nano::lmdb::account_store::del (nano::write_transaction const & transaction_a, nano::account const & account_a)
{
	nano::mdb_val key{ account_a };
	auto status = mdb_del (to_mdb_txn (transaction_a), accounts_handle, key, nullptr);
	nano::assert_success (status);
}

bool nano::lmdb::account_store::exists (nano::transaction const & transaction_a, nano::account const & account_a)
{
	auto iterator (begin (transaction_a, account_a));
	return iterator != end () && nano::account (iterator->first) == account_a;
}

size_t nano::lmdb::account_store::count (nano::transaction const & transaction_a)
{
	return store.count (transaction_a, tables::accounts);
}

nano::store_iterator<nano::account, nano::account_info> nano::lmdb::account_store::begin (nano::transaction const & transaction, nano::account const & account) const
{
	return store.make_iterator<nano::account, nano::account_info> (transaction, tables::accounts, account);
}

nano::store_iterator<nano::account, nano::account_info> nano::lmdb::account_store::begin (nano::transaction const & transaction) const
{
	return store.make_iterator<nano::account, nano::account_info> (transaction, tables::accounts);
}

nano::store_iterator<nano::account, nano::account_info> nano::lmdb::account_store::rbegin (nano::transaction const & transaction_a) const
{
	return store.make_iterator<nano::account, nano::account_info> (transaction_a, tables::accounts, false);
}

nano::store_iterator<nano::account, nano::account_info> nano::lmdb::account_store::end () const
{
	return nano::store_iterator<nano::account, nano::account_info> (nullptr);
}

void nano::lmdb::account_store::for_each_par (std::function<void (nano::read_transaction const &, nano::store_iterator<nano::account, nano::account_info>, nano::store_iterator<nano::account, nano::account_info>)> const & action_a) const
{
	parallel_traversal<nano::uint256_t> (
	[&action_a, this] (nano::uint256_t const & start, nano::uint256_t const & end, bool const is_last) {
		auto transaction (this->store.tx_begin_read ());
		action_a (*transaction, this->begin (*transaction, start), !is_last ? this->begin (*transaction, end) : this->end ());
	});
}
