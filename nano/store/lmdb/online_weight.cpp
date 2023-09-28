#include <nano/store/lmdb/lmdb.hpp>
#include <nano/store/lmdb/online_weight.hpp>

namespace
{
nano::store::iterator<uint64_t, nano::amount> to_iterator (rsnano::LmdbIteratorHandle * it_handle)
{
	if (it_handle == nullptr)
	{
		return { nullptr };
	}

	return { std::make_unique<nano::store::lmdb::iterator<uint64_t, nano::amount>> (it_handle) };
}
}

nano::store::lmdb::online_weight::online_weight (rsnano::LmdbOnlineWeightStoreHandle * handle_a) :
	handle{ handle_a }
{
}

nano::store::lmdb::online_weight::~online_weight ()
{
	if (handle != nullptr)
		rsnano::rsn_lmdb_online_weight_store_destroy (handle);
}

void nano::store::lmdb::online_weight::put (nano::store::write_transaction const & transaction, uint64_t time, nano::amount const & amount)
{
	rsnano::rsn_lmdb_online_weight_store_put (handle, transaction.get_rust_handle (), time, amount.bytes.data ());
}

void nano::store::lmdb::online_weight::del (nano::store::write_transaction const & transaction, uint64_t time)
{
	rsnano::rsn_lmdb_online_weight_store_del (handle, transaction.get_rust_handle (), time);
}

nano::store::iterator<uint64_t, nano::amount> nano::store::lmdb::online_weight::begin (nano::store::transaction const & transaction) const
{
	auto it_handle{ rsnano::rsn_lmdb_online_weight_store_begin (handle, transaction.get_rust_handle ()) };
	return to_iterator (it_handle);
}

nano::store::iterator<uint64_t, nano::amount> nano::store::lmdb::online_weight::rbegin (nano::store::transaction const & transaction) const
{
	auto it_handle{ rsnano::rsn_lmdb_online_weight_store_rbegin (handle, transaction.get_rust_handle ()) };
	return to_iterator (it_handle);
}

nano::store::iterator<uint64_t, nano::amount> nano::store::lmdb::online_weight::end () const
{
	return nano::store::iterator<uint64_t, nano::amount> (nullptr);
}

size_t nano::store::lmdb::online_weight::count (nano::store::transaction const & transaction) const
{
	return rsnano::rsn_lmdb_online_weight_store_count (handle, transaction.get_rust_handle ());
}

void nano::store::lmdb::online_weight::clear (nano::store::write_transaction const & transaction)
{
	return rsnano::rsn_lmdb_online_weight_store_clear (handle, transaction.get_rust_handle ());
}
