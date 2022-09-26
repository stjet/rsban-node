#pragma once

#include <nano/lib/numbers.hpp>
#include <nano/secure/store.hpp>

#include <lmdb/libraries/liblmdb/lmdb.h>

namespace nano
{
class wallet_value
{
public:
	wallet_value () = default;
	wallet_value (nano::db_val<MDB_val> const &);
	wallet_value (nano::raw_key const &, uint64_t);
	explicit wallet_value (rsnano::WalletValueDto const & dto_a);
	nano::db_val<MDB_val> val () const;
	rsnano::WalletValueDto to_dto () const;
	nano::raw_key key;
	uint64_t work;
};
}
