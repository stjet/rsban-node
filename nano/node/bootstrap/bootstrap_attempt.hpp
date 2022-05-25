#pragma once

#include <nano/node/bootstrap/bootstrap.hpp>

#include <atomic>
#include <future>

namespace nano
{
class node;

class frontier_req_client;
class bulk_push_client;

/**
 * Polymorphic base class for bootstrap sessions.
 */
class bootstrap_attempt : public std::enable_shared_from_this<bootstrap_attempt>
{
public:
	explicit bootstrap_attempt (std::shared_ptr<nano::node> const & node_a, nano::bootstrap_mode mode_a, uint64_t incremental_id_a, std::string id_a);
	virtual ~bootstrap_attempt ();
	virtual void run () = 0;
	virtual void stop ();
	bool still_pulling ();
	void pull_started ();
	void pull_finished ();
	bool should_log ();
	std::string mode_text ();
	virtual bool process_block (std::shared_ptr<nano::block> const &, nano::account const &, uint64_t, nano::bulk_pull::count_t, bool, unsigned);
	virtual void get_information (boost::property_tree::ptree &) = 0;
	uint64_t total_blocks () const;
	void total_blocks_inc ();
	unsigned get_pulling () const;
	void inc_pulling ();
	bool get_stopped () const;
	void set_stopped ();

	std::shared_ptr<nano::node> node;
	std::atomic<unsigned> requeued_pulls{ 0 };
	std::atomic<bool> started{ false };
	std::chrono::steady_clock::time_point attempt_start{ std::chrono::steady_clock::now () };
	std::atomic<bool> frontiers_received{ false };
	nano::bootstrap_mode mode;

	std::string id () const;
	uint64_t get_incremental_id () const;
	void notify_all ();

protected:
	rsnano::BootstrapAttemptHandle * handle;
};
}
