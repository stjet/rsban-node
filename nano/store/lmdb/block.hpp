#pragma once

#include <nano/store/block.hpp>
#include <nano/store/lmdb/db_val.hpp>

namespace nano::store::lmdb
{
class block : public nano::store::block
{
	rsnano::LmdbBlockStoreHandle * handle;

public:
	explicit block (rsnano::LmdbBlockStoreHandle * handle_a);
	block (block const &) = delete;
	block (block &&) = delete;
	~block () override;
	void put (nano::store::write_transaction const & transaction_a, nano::block_hash const & hash_a, nano::block const & block_a) override;
	void raw_put (nano::store::write_transaction const & transaction_a, std::vector<uint8_t> const & data, nano::block_hash const & hash_a) override;
	std::optional<nano::block_hash> successor (nano::store::transaction const & transaction_a, nano::block_hash const & hash_a) const override;
	void successor_clear (nano::store::write_transaction const & transaction_a, nano::block_hash const & hash_a) override;
	std::shared_ptr<nano::block> get (nano::store::transaction const & transaction_a, nano::block_hash const & hash_a) const override;
	std::shared_ptr<nano::block> random (nano::store::transaction const & transaction_a) override;
	void del (nano::store::write_transaction const & transaction_a, nano::block_hash const & hash_a) override;
	bool exists (nano::store::transaction const & transaction_a, nano::block_hash const & hash_a) override;
	uint64_t count (nano::store::transaction const & transaction_a) override;
};
}
