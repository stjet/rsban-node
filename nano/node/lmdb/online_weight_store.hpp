#pragma once

#include <nano/secure/store.hpp>

#include <lmdb/libraries/liblmdb/lmdb.h>

namespace nano
{
namespace lmdb
{
	class online_weight_store : public nano::online_weight_store
	{
	private:
		rsnano::LmdbOnlineWeightStoreHandle * handle;
		nano::lmdb::store & store;

	public:
		explicit online_weight_store (nano::lmdb::store & store_a);
		~online_weight_store ();
		online_weight_store (online_weight_store const &) = delete;
		online_weight_store (online_weight_store &&) = delete;
		void put (nano::write_transaction const & transaction_a, uint64_t time_a, nano::amount const & amount_a) override;
		void del (nano::write_transaction const & transaction_a, uint64_t time_a) override;
		nano::store_iterator<uint64_t, nano::amount> begin (nano::transaction const & transaction_a) const override;
		nano::store_iterator<uint64_t, nano::amount> rbegin (nano::transaction const & transaction_a) const override;
		nano::store_iterator<uint64_t, nano::amount> end () const override;
		size_t count (nano::transaction const & transaction_a) const override;
		void clear (nano::write_transaction const & transaction_a) override;

		MDB_dbi table_handle () const;
		void set_table_handle (MDB_dbi dbi);
	};
}
}
