#include <nano/store/lmdb/lmdb.hpp>
#include <nano/store/lmdb/peer.hpp>

namespace
{
nano::store::iterator<nano::endpoint_key, nano::no_value> to_iterator (rsnano::LmdbIteratorHandle * it_handle)
{
	if (it_handle == nullptr)
	{
		return { nullptr };
	}

	return { std::make_unique<nano::store::lmdb::iterator<nano::endpoint_key, nano::no_value>> (it_handle) };
}
}

nano::store::lmdb::peer::peer (rsnano::LmdbPeerStoreHandle * handle_a) :
	handle{ handle_a }
{
}

nano::store::lmdb::peer::~peer ()
{
	if (handle != nullptr)
		rsnano::rsn_lmdb_peer_store_destroy (handle);
}

size_t nano::store::lmdb::peer::count (nano::store::transaction const & transaction) const
{
	return rsnano::rsn_lmdb_peer_store_count (handle, transaction.get_rust_handle ());
}

void nano::store::lmdb::peer::clear (nano::store::write_transaction const & transaction)
{
	rsnano::rsn_lmdb_peer_store_clear (handle, transaction.get_rust_handle ());
}
