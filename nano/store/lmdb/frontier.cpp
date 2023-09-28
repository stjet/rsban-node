#include <nano/store/lmdb/frontier.hpp>
#include <nano/store/lmdb/lmdb.hpp>

nano::store::lmdb::frontier::frontier (rsnano::LmdbFrontierStoreHandle * handle_a) :
	handle{ handle_a }
{
}

nano::store::lmdb::frontier::~frontier ()
{
	if (handle != nullptr)
		rsnano::rsn_lmdb_frontier_store_destroy (handle);
}

void nano::store::lmdb::frontier::put (nano::store::write_transaction const & transaction, nano::block_hash const & hash, nano::account const & account)
{
	rsnano::rsn_lmdb_frontier_store_put (handle, transaction.get_rust_handle (), hash.bytes.data (), account.bytes.data ());
}

nano::account nano::store::lmdb::frontier::get (nano::store::transaction const & transaction, nano::block_hash const & hash) const
{
	nano::account result;
	rsnano::rsn_lmdb_frontier_store_get (handle, transaction.get_rust_handle (), hash.bytes.data (), result.bytes.data ());
	return result;
}

void nano::store::lmdb::frontier::del (nano::store::write_transaction const & transaction, nano::block_hash const & hash)
{
	rsnano::rsn_lmdb_frontier_store_del (handle, transaction.get_rust_handle (), hash.bytes.data ());
}

namespace
{
nano::store::iterator<nano::block_hash, nano::account> to_iterator (rsnano::LmdbIteratorHandle * it_handle)
{
	if (it_handle == nullptr)
	{
		return { nullptr };
	}

	return { std::make_unique<nano::store::lmdb::iterator<nano::block_hash, nano::account>> (it_handle) };
}
}

nano::store::iterator<nano::block_hash, nano::account> nano::store::lmdb::frontier::begin (nano::store::transaction const & transaction) const
{
	auto it_handle{ rsnano::rsn_lmdb_frontier_store_begin (handle, transaction.get_rust_handle ()) };
	return to_iterator (it_handle);
}

nano::store::iterator<nano::block_hash, nano::account> nano::store::lmdb::frontier::begin (nano::store::transaction const & transaction, nano::block_hash const & hash) const
{
	auto it_handle{ rsnano::rsn_lmdb_frontier_store_begin_at_hash (handle, transaction.get_rust_handle (), hash.bytes.data ()) };
	return to_iterator (it_handle);
}

nano::store::iterator<nano::block_hash, nano::account> nano::store::lmdb::frontier::end () const
{
	return { nullptr };
}

namespace
{
void for_each_par_wrapper (void * context, rsnano::TransactionHandle * txn_handle, rsnano::LmdbIteratorHandle * begin_handle, rsnano::LmdbIteratorHandle * end_handle)
{
	auto action = static_cast<std::function<void (nano::store::read_transaction const &, nano::store::iterator<nano::block_hash, nano::account>, nano::store::iterator<nano::block_hash, nano::account>)> const *> (context);
	nano::store::lmdb::read_transaction_impl txn{ txn_handle };
	auto begin{ to_iterator (begin_handle) };
	auto end{ to_iterator (end_handle) };
	(*action) (txn, std::move (begin), std::move (end));
}
void for_each_par_delete_context (void * context)
{
}
}

void nano::store::lmdb::frontier::for_each_par (std::function<void (nano::store::read_transaction const &, nano::store::iterator<nano::block_hash, nano::account>, nano::store::iterator<nano::block_hash, nano::account>)> const & action_a) const
{
	auto context = (void *)&action_a;
	rsnano::rsn_lmdb_frontier_store_for_each_par (handle, for_each_par_wrapper, context, for_each_par_delete_context);
}
