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

void nano::store::lmdb::peer::put (nano::store::write_transaction const & transaction, nano::endpoint_key const & endpoint)
{
	rsnano::rsn_lmdb_peer_store_put (handle, transaction.get_rust_handle (), endpoint.address_bytes ().data (), endpoint.port ());
}

bool nano::store::lmdb::peer::exists (nano::store::transaction const & transaction, nano::endpoint_key const & endpoint) const
{
	return rsnano::rsn_lmdb_peer_store_exists (handle, transaction.get_rust_handle (), endpoint.address_bytes ().data (), endpoint.port ());
}

size_t nano::store::lmdb::peer::count (nano::store::transaction const & transaction) const
{
	return rsnano::rsn_lmdb_peer_store_count (handle, transaction.get_rust_handle ());
}

void nano::store::lmdb::peer::clear (nano::store::write_transaction const & transaction)
{
	rsnano::rsn_lmdb_peer_store_clear (handle, transaction.get_rust_handle ());
}
