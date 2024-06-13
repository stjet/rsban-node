#pragma once

#include <nano/node/bootstrap/bootstrap.hpp>

#include <atomic>
#include <future>

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
	virtual void run ();
	virtual void stop ();
	virtual void get_information (boost::property_tree::ptree &) = 0;
	virtual void block_processed (nano::store::transaction const & tx, nano::block_status const & result, nano::block const & block);
	uint64_t total_blocks () const;
	void total_blocks_inc ();
	unsigned get_pulling () const;
	void inc_pulling ();
	bool get_stopped () const;
	bool get_started () const;
	unsigned get_requeued_pulls () const;
	bool get_frontiers_received () const;

	std::string id () const;
	rsnano::BootstrapAttemptHandle * handle;
};
}
