#include <nano/node/lmdb/block_store.hpp>
#include <nano/node/lmdb/lmdb.hpp>
#include <nano/secure/parallel_traversal.hpp>

namespace nano
{
size_t block_successor_offset (nano::transaction const & transaction_a, size_t entry_size_a, nano::block_type type_a)
{
	return entry_size_a - nano::block_sideband::size (type_a);
}
}

nano::lmdb::block_store::block_store (nano::lmdb::store & store_a) :
	handle{ rsnano::rsn_lmdb_block_store_create (store_a.env ().handle) },
	store{ store_a } {};

nano::lmdb::block_store::~block_store ()
{
	rsnano::rsn_lmdb_block_store_destroy (handle);
}

void nano::lmdb::block_store::put (nano::write_transaction const & transaction, nano::block_hash const & hash, nano::block const & block)
{
	rsnano::rsn_lmdb_block_store_put (handle, transaction.get_rust_handle (), hash.bytes.data (), block.get_handle ());
}

void nano::lmdb::block_store::raw_put (nano::write_transaction const & transaction_a, std::vector<uint8_t> const & data, nano::block_hash const & hash_a)
{
	rsnano::rsn_lmdb_block_store_raw_put (handle, transaction_a.get_rust_handle (), data.data (), data.size (), hash_a.bytes.data ());
}

nano::block_hash nano::lmdb::block_store::successor (nano::transaction const & transaction_a, nano::block_hash const & hash_a) const
{
	nano::block_hash result;
	rsnano::rsn_lmdb_block_store_successor (handle, transaction_a.get_rust_handle (), hash_a.bytes.data (), result.bytes.data ());
	return result;
}

void nano::lmdb::block_store::successor_clear (nano::write_transaction const & transaction, nano::block_hash const & hash)
{
	nano::mdb_val value;
	block_raw_get (transaction, hash, value);
	debug_assert (value.size () != 0);
	auto type = block_type_from_raw (value.data ());
	std::vector<uint8_t> data (static_cast<uint8_t *> (value.data ()), static_cast<uint8_t *> (value.data ()) + value.size ());
	std::fill_n (data.begin () + block_successor_offset (transaction, value.size (), type), sizeof (nano::block_hash), uint8_t{ 0 });
	raw_put (transaction, data, hash);
}

std::shared_ptr<nano::block> nano::lmdb::block_store::get (nano::transaction const & transaction, nano::block_hash const & hash) const
{
	nano::mdb_val value;
	block_raw_get (transaction, hash, value);
	std::shared_ptr<nano::block> result;
	if (value.size () != 0)
	{
		nano::bufferstream stream (reinterpret_cast<uint8_t const *> (value.data ()), value.size ());
		nano::block_type type;
		auto error (try_read (stream, type));
		release_assert (!error);
		result = nano::deserialize_block (stream, type);
		release_assert (result != nullptr);
		nano::block_sideband sideband;
		error = (sideband.deserialize (stream, type));
		release_assert (!error);
		result->sideband_set (sideband);
	}
	return result;
}

std::shared_ptr<nano::block> nano::lmdb::block_store::get_no_sideband (nano::transaction const & transaction, nano::block_hash const & hash) const
{
	nano::mdb_val value;
	block_raw_get (transaction, hash, value);
	std::shared_ptr<nano::block> result;
	if (value.size () != 0)
	{
		nano::bufferstream stream (reinterpret_cast<uint8_t const *> (value.data ()), value.size ());
		result = nano::deserialize_block (stream);
		debug_assert (result != nullptr);
	}
	return result;
}

std::shared_ptr<nano::block> nano::lmdb::block_store::random (nano::transaction const & transaction)
{
	nano::block_hash hash;
	nano::random_pool::generate_block (hash.bytes.data (), hash.bytes.size ());
	auto existing = begin (transaction, hash);
	if (existing == end ())
	{
		existing = begin (transaction);
	}
	debug_assert (existing != end ());
	return existing->second.block;
}

void nano::lmdb::block_store::del (nano::write_transaction const & transaction_a, nano::block_hash const & hash_a)
{
	auto status = store.del (transaction_a, tables::blocks, hash_a);
	store.release_assert_success (status);
}

bool nano::lmdb::block_store::exists (nano::transaction const & transaction, nano::block_hash const & hash)
{
	return rsnano::rsn_lmdb_block_store_exists (handle, transaction.get_rust_handle (), hash.bytes.data ());
}

uint64_t nano::lmdb::block_store::count (nano::transaction const & transaction_a)
{
	return store.count (transaction_a, tables::blocks);
}

nano::account nano::lmdb::block_store::account (nano::transaction const & transaction_a, nano::block_hash const & hash_a) const
{
	auto block (get (transaction_a, hash_a));
	debug_assert (block != nullptr);
	return account_calculated (*block);
}

nano::account nano::lmdb::block_store::account_calculated (nano::block const & block_a) const
{
	debug_assert (block_a.has_sideband ());
	nano::account result (block_a.account ());
	if (result.is_zero ())
	{
		result = block_a.sideband ().account ();
	}
	debug_assert (!result.is_zero ());
	return result;
}

nano::store_iterator<nano::block_hash, nano::block_w_sideband> nano::lmdb::block_store::begin (nano::transaction const & transaction) const
{
	return store.make_iterator<nano::block_hash, nano::block_w_sideband> (transaction, tables::blocks);
}

nano::store_iterator<nano::block_hash, nano::block_w_sideband> nano::lmdb::block_store::begin (nano::transaction const & transaction, nano::block_hash const & hash) const
{
	return store.make_iterator<nano::block_hash, nano::block_w_sideband> (transaction, tables::blocks, hash);
}

nano::store_iterator<nano::block_hash, nano::block_w_sideband> nano::lmdb::block_store::end () const
{
	return nano::store_iterator<nano::block_hash, nano::block_w_sideband> (nullptr);
}

nano::uint128_t nano::lmdb::block_store::balance (nano::transaction const & transaction_a, nano::block_hash const & hash_a)
{
	auto block (get (transaction_a, hash_a));
	release_assert (block);
	nano::uint128_t result (balance_calculated (block));
	return result;
}

nano::uint128_t nano::lmdb::block_store::balance_calculated (std::shared_ptr<nano::block> const & block_a) const
{
	nano::uint128_t result;
	switch (block_a->type ())
	{
		case nano::block_type::open:
		case nano::block_type::receive:
		case nano::block_type::change:
			result = block_a->sideband ().balance ().number ();
			break;
		case nano::block_type::send:
			result = boost::polymorphic_downcast<nano::send_block *> (block_a.get ())->balance ().number ();
			break;
		case nano::block_type::state:
			result = boost::polymorphic_downcast<nano::state_block *> (block_a.get ())->balance ().number ();
			break;
		case nano::block_type::invalid:
		case nano::block_type::not_a_block:
			release_assert (false);
			break;
	}
	return result;
}

nano::epoch nano::lmdb::block_store::version (nano::transaction const & transaction_a, nano::block_hash const & hash_a)
{
	auto block = get (transaction_a, hash_a);
	if (block && block->type () == nano::block_type::state)
	{
		return block->sideband ().details ().epoch ();
	}

	return nano::epoch::epoch_0;
}

void nano::lmdb::block_store::for_each_par (std::function<void (nano::read_transaction const &, nano::store_iterator<nano::block_hash, block_w_sideband>, nano::store_iterator<nano::block_hash, block_w_sideband>)> const & action_a) const
{
	parallel_traversal<nano::uint256_t> (
	[&action_a, this] (nano::uint256_t const & start, nano::uint256_t const & end, bool const is_last) {
		auto transaction (this->store.tx_begin_read ());
		action_a (*transaction, this->begin (*transaction, start), !is_last ? this->begin (*transaction, end) : this->end ());
	});
}

// Converts a block hash to a block height
uint64_t nano::lmdb::block_store::account_height (nano::transaction const & transaction_a, nano::block_hash const & hash_a) const
{
	auto block = get (transaction_a, hash_a);
	return block->sideband ().height ();
}

MDB_dbi nano::lmdb::block_store::get_blocks_handle () const
{
	return rsnano::rsn_lmdb_block_store_blocks_handle (handle);
}

void nano::lmdb::block_store::set_blocks_handle (MDB_dbi dbi)
{
	rsnano::rsn_lmdb_block_store_set_blocks_handle (handle, dbi);
}

void nano::lmdb::block_store::block_raw_get (nano::transaction const & transaction, nano::block_hash const & hash, nano::mdb_val & value) const
{
	rsnano::rsn_lmdb_block_store_block_raw_get (handle, transaction.get_rust_handle (), hash.bytes.data (), reinterpret_cast<rsnano::MdbVal *> (&value.value));
}

nano::block_type nano::lmdb::block_store::block_type_from_raw (void * data_a)
{
	// The block type is the first byte
	return static_cast<nano::block_type> ((reinterpret_cast<uint8_t const *> (data_a))[0]);
}
