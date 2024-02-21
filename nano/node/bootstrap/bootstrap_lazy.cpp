#include "nano/lib/rsnano.hpp"
#include <nano/node/bootstrap/bootstrap.hpp>
#include <nano/node/bootstrap/bootstrap_lazy.hpp>
#include <nano/node/common.hpp>
#include <nano/node/node.hpp>

#include <boost/format.hpp>

#include <algorithm>

constexpr std::chrono::seconds nano::bootstrap_limits::lazy_flush_delay_sec;
constexpr uint64_t nano::bootstrap_limits::lazy_batch_pull_count_resize_blocks_limit;
constexpr double nano::bootstrap_limits::lazy_batch_pull_count_resize_ratio;
constexpr std::size_t nano::bootstrap_limits::lazy_blocks_restart_limit;

namespace
{
rsnano::BootstrapAttemptHandle * create_lazy_handle(nano::bootstrap_attempt_lazy * self, std::shared_ptr<nano::node> const & node_a, uint64_t incremental_id_a, std::string const & id_a)
{
	auto network_params_dto{node_a->network_params.to_dto()};
	return rsnano::rsn_bootstrap_attempt_lazy_create (
			self, 
			node_a->websocket.server.get (), 
			node_a->block_processor.get_handle (), 
			node_a->bootstrap_initiator.get_handle (), 
			node_a->ledger.get_handle (), 
			id_a.c_str (), 
			incremental_id_a,
			node_a->flags.handle,
			node_a->bootstrap_initiator.connections->handle,
			&network_params_dto);
}
}

nano::bootstrap_attempt_lazy::bootstrap_attempt_lazy (std::shared_ptr<nano::node> const & node_a, uint64_t incremental_id_a, std::string const & id_a) :
	nano::bootstrap_attempt (create_lazy_handle(this, node_a, incremental_id_a, id_a))
{
}

bool nano::bootstrap_attempt_lazy::lazy_start (nano::hash_or_account const & hash_or_account_a)
{
	return rsnano::rsn_bootstrap_attempt_lazy_lazy_start(handle, hash_or_account_a.bytes.data());
}

void nano::bootstrap_attempt_lazy::lazy_add (nano::pull_info const & pull_a)
{
	auto pull_dto {pull_a.to_dto()};
	rsnano::rsn_bootstrap_attempt_lazy_lazy_add(handle, &pull_dto);
}

void nano::bootstrap_attempt_lazy::lazy_requeue (nano::block_hash const & hash_a, nano::block_hash const & previous_a)
{
	rsnano::rsn_bootstrap_attempt_lazy_lazy_requeue(handle, hash_a.bytes.data(), previous_a.bytes.data());
}

uint32_t nano::bootstrap_attempt_lazy::lazy_batch_size ()
{
	return rsnano::rsn_bootstrap_attempt_lazy_lazy_batch_size(handle);
}

bool nano::bootstrap_attempt_lazy::lazy_processed_or_exists (nano::block_hash const & hash_a)
{
	return rsnano::rsn_bootstrap_attempt_lazy_lazy_processed_or_exists(handle, hash_a.bytes.data());
}

void nano::bootstrap_attempt_lazy::get_information (boost::property_tree::ptree & tree_a)
{
	rsnano::rsn_bootstrap_attempt_lazy_get_information(handle, &tree_a);
}

nano::bootstrap_attempt_wallet::bootstrap_attempt_wallet (std::shared_ptr<nano::node> const & node_a, uint64_t incremental_id_a, std::string id_a) :
	nano::bootstrap_attempt (node_a, nano::bootstrap_mode::wallet_lazy, incremental_id_a, id_a),
	node_weak (node_a)
{
}

nano::bootstrap_attempt_wallet::~bootstrap_attempt_wallet ()
{
}

rsnano::BootstrapAttemptLockHandle * nano::bootstrap_attempt_wallet::request_pending (rsnano::BootstrapAttemptLockHandle * lock_a)
{
	auto node = node_weak.lock ();
	if (!node || node->is_stopped ())
	{
		return lock_a;
	}
	rsnano::rsn_bootstrap_attempt_unlock (lock_a);
	auto connection_l (node->bootstrap_initiator.connections->connection (shared_from_this ()));
	lock_a = rsnano::rsn_bootstrap_attempt_lock (handle);
	if (connection_l && !get_stopped ())
	{
		auto account (wallet_accounts.front ());
		wallet_accounts.pop_front ();
		inc_pulling ();
		auto this_l = std::dynamic_pointer_cast<nano::bootstrap_attempt_wallet> (shared_from_this ());
		// The bulk_pull_account_client destructor attempt to requeue_pull which can cause a deadlock if this is the last reference
		// Dispatch request in an external thread in case it needs to be destroyed
		node->background ([connection_l, this_l, account, node] () {
			auto client (std::make_shared<nano::bulk_pull_account_client> (node, connection_l, this_l, account));
			client->request ();
		});
	}
	return lock_a;
}

void nano::bootstrap_attempt_wallet::requeue_pending (nano::account const & account_a)
{
	auto account (account_a);
	{
		auto lock{ rsnano::rsn_bootstrap_attempt_lock (handle) };
		wallet_accounts.push_front (account);
		rsnano::rsn_bootstrap_attempt_unlock (lock);
	}
	rsnano::rsn_bootstrap_attempt_notifiy_all (handle);
}

void nano::bootstrap_attempt_wallet::wallet_start (std::deque<nano::account> & accounts_a)
{
	{
		auto lock{ rsnano::rsn_bootstrap_attempt_lock (handle) };
		wallet_accounts.swap (accounts_a);
		rsnano::rsn_bootstrap_attempt_unlock (lock);
	}
	rsnano::rsn_bootstrap_attempt_notifiy_all (handle);
}

bool nano::bootstrap_attempt_wallet::wallet_finished ()
{
	// debug_assert (!mutex.try_lock ());
	auto running (!get_stopped ());
	auto more_accounts (!wallet_accounts.empty ());
	auto still_pulling (get_pulling () > 0);
	return running && (more_accounts || still_pulling);
}

void nano::bootstrap_attempt_wallet::run ()
{
	auto node = node_weak.lock ();
	if (!node || node->is_stopped ())
		if (!node)
		{
			return;
		}
	debug_assert (get_started ());
	debug_assert (!node->flags.disable_wallet_bootstrap ());
	node->bootstrap_initiator.connections->populate_connections (false);
	auto start_time (std::chrono::steady_clock::now ());
	auto max_time (std::chrono::minutes (10));
	auto lock{ rsnano::rsn_bootstrap_attempt_lock (handle) };
	while (wallet_finished () && std::chrono::steady_clock::now () - start_time < max_time)
	{
		if (!wallet_accounts.empty ())
		{
			lock = request_pending (lock);
		}
		else
		{
			rsnano::rsn_bootstrap_attempt_wait_for (handle, lock, 1000);
		}
	}
	if (!get_stopped ())
	{
		node->logger->info (nano::log::type::bootstrap_lazy, "Completed wallet lazy pulls");
	}
	rsnano::rsn_bootstrap_attempt_unlock (lock);
	stop ();
	rsnano::rsn_bootstrap_attempt_notifiy_all (handle);
}

std::size_t nano::bootstrap_attempt_wallet::wallet_size ()
{
	auto lock{ rsnano::rsn_bootstrap_attempt_lock (handle) };
	auto size{ wallet_accounts.size () };
	rsnano::rsn_bootstrap_attempt_unlock (lock);
	return size;
}

void nano::bootstrap_attempt_wallet::get_information (boost::property_tree::ptree & tree_a)
{
	auto lock{ rsnano::rsn_bootstrap_attempt_lock (handle) };
	tree_a.put ("wallet_accounts", std::to_string (wallet_accounts.size ()));
	rsnano::rsn_bootstrap_attempt_unlock (lock);
}
