#pragma once

#include <nano/secure/store.hpp>

#include <lmdb/libraries/liblmdb/lmdb.h>

namespace nano
{
namespace lmdb
{
	class store;
	class unchecked_store : public nano::unchecked_store
	{
	private:
		nano::lmdb::store & store;
		rsnano::LmdbUncheckedStoreHandle * handle;

	public:
		unchecked_store (nano::lmdb::store & store_a);
		~unchecked_store ();
		unchecked_store (unchecked_store const &) = delete;
		unchecked_store (unchecked_store &&) = delete;

		void clear (nano::write_transaction const & transaction_a) override;
		void put (nano::write_transaction const & transaction_a, nano::hash_or_account const & dependency, nano::unchecked_info const & info_a) override;
		bool exists (nano::transaction const & transaction_a, nano::unchecked_key const & unchecked_key_a) override;
		void del (nano::write_transaction const & transaction_a, nano::unchecked_key const & key_a) override;
		nano::store_iterator<nano::unchecked_key, nano::unchecked_info> end () const override;
		nano::store_iterator<nano::unchecked_key, nano::unchecked_info> begin (nano::transaction const & transaction_a) const override;
		nano::store_iterator<nano::unchecked_key, nano::unchecked_info> lower_bound (nano::transaction const & transaction_a, nano::unchecked_key const & key_a) const override;
		size_t count (nano::transaction const & transaction_a) override;

		MDB_dbi table_handle () const;
		void set_table_handle (MDB_dbi dbi);
	};
}
}
