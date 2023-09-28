#pragma once

#include <nano/store/version.hpp>

namespace nano::store::lmdb
{
class version : public nano::store::version
{
protected:
	rsnano::LmdbVersionStoreHandle * handle;

public:
	explicit version (rsnano::LmdbVersionStoreHandle * handle_a);
	~version ();
	version (version const &) = delete;
	version (version &&) = delete;
	void put (nano::store::write_transaction const & transaction_a, int version_a) override;
	int get (nano::store::transaction const & transaction_a) const override;
};
}
