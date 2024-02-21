#pragma once

#include <nano/node/bootstrap/bootstrap_attempt.hpp>

#include <boost/multi_index_container.hpp>

#include <atomic>
#include <unordered_set>

namespace nano
{
class node;
class lazy_state_backlog_item final
{
public:
	nano::link link{ 0 };
	nano::uint128_t balance{ 0 };
	unsigned retry_limit{ 0 };
};

/**
 * Lazy bootstrap session. Started with a block hash, this will "trace down" the blocks obtained to find a connection to the ledger.
 * This attempts to quickly bootstrap a section of the ledger given a hash that's known to be confirmed.
 */
class bootstrap_attempt_lazy final : public bootstrap_attempt
{
public:
	explicit bootstrap_attempt_lazy (std::shared_ptr<nano::node> const & node_a, uint64_t incremental_id_a, std::string const & id_a = "");
	bool lazy_start (nano::hash_or_account const &);
	void lazy_add (nano::pull_info const &);
	void lazy_requeue (nano::block_hash const &, nano::block_hash const &);
	uint32_t lazy_batch_size ();
	bool lazy_processed_or_exists (nano::block_hash const &);
	void get_information (boost::property_tree::ptree &) override;
};

/**
 * Wallet bootstrap session. This session will trace down accounts within local wallets to try and bootstrap those blocks first.
 */
class bootstrap_attempt_wallet final : public bootstrap_attempt
{
public:
	explicit bootstrap_attempt_wallet (std::shared_ptr<nano::node> const & node_a, uint64_t incremental_id_a, std::string id_a = "");
	~bootstrap_attempt_wallet ();
	rsnano::BootstrapAttemptLockHandle * request_pending (rsnano::BootstrapAttemptLockHandle * lock_a);
	void requeue_pending (nano::account const &);
	void run () override;
	void wallet_start (std::deque<nano::account> &);
	bool wallet_finished ();
	std::size_t wallet_size ();
	void get_information (boost::property_tree::ptree &) override;
	std::weak_ptr<nano::node> node_weak;
	std::deque<nano::account> wallet_accounts;
};
}
