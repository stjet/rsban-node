#pragma once

#include <nano/lib/numbers.hpp>

#include <chrono>

namespace rsnano
{
class ConfirmingSetHandle;
}

namespace nano
{
class block;
class ledger;
class container_info_component;
}
namespace nano::store
{
class write_queue;
}

namespace nano
{
/**
 * Set of blocks to be durably confirmed
 */
class confirming_set final
{
public:
	confirming_set (nano::ledger & ledger, nano::store::write_queue & write_queue, std::chrono::milliseconds batch_time = std::chrono::milliseconds{ 500 });
	~confirming_set ();
	// Adds a block to the set of blocks to be confirmed
	void add (nano::block_hash const & hash);
	void start ();
	void stop ();
	// Added blocks will remain in this set until after ledger has them marked as confirmed.
	bool exists (nano::block_hash const & hash) const;
	std::size_t size () const;
	std::unique_ptr<container_info_component> collect_container_info (std::string const & name) const;

	// Observers will be called once ledger has blocks marked as confirmed
	void add_cemented_observer (std::function<void (std::shared_ptr<nano::block> const &)> const &);
	void add_block_already_cemented_observer (std::function<void (nano::block_hash const &)> const &);
	rsnano::ConfirmingSetHandle * handle;
};
}
