#pragma once

#include <nano/node/bootstrap/bootstrap.hpp>

namespace nano::store
{
class transaction;
}

namespace nano
{
class node;

/**
 * Polymorphic base class for bootstrap sessions.
 */
class bootstrap_attempt : public std::enable_shared_from_this<bootstrap_attempt>
{
public:
	explicit bootstrap_attempt (std::shared_ptr<nano::node> const & node_a, nano::bootstrap_mode mode_a, uint64_t incremental_id_a, std::string id_a);
	explicit bootstrap_attempt (rsnano::BootstrapAttemptHandle * handle);
	virtual ~bootstrap_attempt ();
	uint64_t total_blocks () const;
	void total_blocks_inc ();
	unsigned get_pulling () const;
	void inc_pulling ();
	bool get_stopped () const;
	bool get_started () const;
	unsigned get_requeued_pulls () const;

	std::string id () const;
	rsnano::BootstrapAttemptHandle * handle;
};
}
