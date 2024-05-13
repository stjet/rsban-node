#pragma once

#include <nano/node/bootstrap/bootstrap_attempt.hpp>

#include <boost/property_tree/ptree_fwd.hpp>

#include <memory>

namespace nano
{
class node;

/**
 * Legacy bootstrap session. This is made up of 3 phases: frontier requests, bootstrap pulls, bootstrap pushes.
 */
class bootstrap_attempt_legacy : public bootstrap_attempt
{
public:
	explicit bootstrap_attempt_legacy (std::shared_ptr<nano::node> const & node_a, uint64_t const incremental_id_a, std::string const & id_a, uint32_t const frontiers_age_a, nano::account const & start_account_a);
	void add_frontier (nano::pull_info const &);
	void add_bulk_push_target (nano::block_hash const &, nano::block_hash const &);
	bool request_bulk_push_target (std::pair<nano::block_hash, nano::block_hash> &);
	void set_start_account (nano::account const &);
	void get_information (boost::property_tree::ptree &) override;
};
}
