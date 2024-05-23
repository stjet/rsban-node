#include "nano/lib/rsnano.hpp"
#include "nano/lib/rsnanoutils.hpp"

#include <nano/lib/threading.hpp>
#include <nano/node/bootstrap/bootstrap.hpp>
#include <nano/node/bootstrap/bootstrap_lazy.hpp>
#include <nano/node/bootstrap/bootstrap_legacy.hpp>
#include <nano/node/common.hpp>
#include <nano/node/node.hpp>

#include <cassert>
#include <memory>

namespace
{
std::shared_ptr<nano::bootstrap_attempt> attempt_from_handle (rsnano::BootstrapAttemptHandle * attempt_handle)
{
	std::shared_ptr<nano::bootstrap_attempt> result{};
	if (attempt_handle)
	{
		auto mode = static_cast<nano::bootstrap_mode> (rsnano::rsn_bootstrap_attempt_bootstrap_mode (attempt_handle));
		switch (mode)
		{
			case nano::bootstrap_mode::lazy:
				result = std::make_shared<nano::bootstrap_attempt_lazy> (attempt_handle);
				break;
			case nano::bootstrap_mode::legacy:
				result = std::make_shared<nano::bootstrap_attempt_legacy> (attempt_handle);
				break;
			case nano::bootstrap_mode::wallet_lazy:
				result = std::make_shared<nano::bootstrap_attempt_wallet> (attempt_handle);
				break;
			default:
				assert (false);
				break;
		}
	}
	return result;
}

rsnano::BootstrapInitiatorHandle * create_bootstrap_initiator (nano::node & node_a)
{
	auto config_dto{ node_a.config->to_dto () };
	auto params_dto{ node_a.network_params.to_dto () };

	return rsnano::rsn_bootstrap_initiator_create (&config_dto,
	node_a.flags.handle, node_a.network->tcp_channels->handle,
	node_a.async_rt.handle, node_a.workers->handle,
	&params_dto,
	new std::weak_ptr<nano::node_observers> (node_a.observers),
	node_a.stats->handle, node_a.outbound_limiter.handle,
	node_a.block_processor.handle, node_a.websocket.get_handle (),
	node_a.ledger.handle);
}
}

nano::bootstrap_initiator::bootstrap_initiator (nano::node & node_a) :
	handle{ create_bootstrap_initiator (node_a) },
	attempts{ rsnano::rsn_bootstrap_initiator_attempts (handle) },
	connections{ std::make_shared<nano::bootstrap_connections> (rsnano::rsn_bootstrap_initiator_connections (handle)) },
	cache{ rsnano::rsn_bootstrap_initiator_cache (handle) }
{
	rsnano::rsn_bootstrap_initiator_initialize (handle);
	rsnano::rsn_bootstrap_initiator_start (handle);
}

nano::bootstrap_initiator::bootstrap_initiator (rsnano::BootstrapInitiatorHandle * handle) :
	handle{ handle },
	attempts{ rsnano::rsn_bootstrap_initiator_attempts (handle) },
	connections{ std::make_shared<nano::bootstrap_connections> (rsnano::rsn_bootstrap_initiator_connections (handle)) },
	cache{ rsnano::rsn_bootstrap_initiator_cache (handle) }
{
}

nano::bootstrap_initiator::~bootstrap_initiator ()
{
	stop ();
	rsnano::rsn_bootstrap_initiator_destroy (handle);
}

void nano::bootstrap_initiator::bootstrap (bool force, std::string id_a, uint32_t const frontiers_age_a, nano::account const & start_account_a)
{
	rsnano::rsn_bootstrap_initiator_bootstrap (handle, force, id_a.c_str (), frontiers_age_a, start_account_a.bytes.data ());
}

void nano::bootstrap_initiator::bootstrap (nano::endpoint const & endpoint_a, bool add_to_peers, std::string id_a)
{
	auto dto{ rsnano::udp_endpoint_to_dto (endpoint_a) };
	rsnano::rsn_bootstrap_initiator_bootstrap2 (handle, &dto, add_to_peers, id_a.c_str ());
}

bool nano::bootstrap_initiator::bootstrap_lazy (nano::hash_or_account const & hash_or_account_a, bool force, std::string id_a)
{
	return rsnano::rsn_bootstrap_initiator_bootstrap_lazy (handle, hash_or_account_a.bytes.data (), force, id_a.c_str ());
}

void nano::bootstrap_initiator::bootstrap_wallet (std::deque<nano::account> & accounts_a)
{
	rsnano::account_vec acc_vec{ accounts_a };
	rsnano::rsn_bootstrap_initiator_bootstrap_wallet (handle, acc_vec.handle);
}

bool nano::bootstrap_initiator::in_progress ()
{
	return rsnano::rsn_bootstrap_initiator_in_progress (handle);
}

std::shared_ptr<nano::bootstrap_attempt> nano::bootstrap_initiator::current_attempt ()
{
	auto attempt_handle = rsnano::rsn_bootstrap_initiator_current_attempt (handle);
	return attempt_from_handle (attempt_handle);
}

std::shared_ptr<nano::bootstrap_attempt_lazy> nano::bootstrap_initiator::current_lazy_attempt ()
{
	auto attempt_handle = rsnano::rsn_bootstrap_initiator_current_lazy_attempt (handle);
	return std::dynamic_pointer_cast<nano::bootstrap_attempt_lazy> (attempt_from_handle (attempt_handle));
}

std::shared_ptr<nano::bootstrap_attempt_wallet> nano::bootstrap_initiator::current_wallet_attempt ()
{
	auto attempt_handle = rsnano::rsn_bootstrap_initiator_current_wallet_attempt (handle);
	return std::dynamic_pointer_cast<nano::bootstrap_attempt_wallet> (attempt_from_handle (attempt_handle));
}

rsnano::BootstrapInitiatorHandle * nano::bootstrap_initiator::get_handle () const
{
	return handle;
}

void nano::bootstrap_initiator::stop ()
{
	rsnano::rsn_bootstrap_initiator_stop (handle);
}

nano::pulls_cache::pulls_cache (rsnano::PullsCacheHandle * handle) :
	handle{ handle }
{
}

nano::pulls_cache::pulls_cache () :
	handle{ rsnano::rsn_pulls_cache_create () }
{
}

nano::pulls_cache::~pulls_cache ()
{
	rsnano::rsn_pulls_cache_destroy (handle);
}

void nano::pulls_cache::add (nano::pull_info const & pull_a)
{
	auto dto{ pull_a.to_dto () };
	rsnano::rsn_pulls_cache_add (handle, &dto);
}

void nano::pulls_cache::update_pull (nano::pull_info & pull_a)
{
	auto dto{ pull_a.to_dto () };
	rsnano::rsn_pulls_cache_update_pull (handle, &dto);
	pull_a.load_dto (dto);
}

void nano::pulls_cache::remove (nano::pull_info const & pull_a)
{
	auto dto{ pull_a.to_dto () };
	rsnano::rsn_pulls_cache_remove (handle, &dto);
}

size_t nano::pulls_cache::size ()
{
	return rsnano::rsn_pulls_cache_size (handle);
}
size_t nano::pulls_cache::element_size ()
{
	return rsnano::rsn_pulls_cache_element_size ();
}

nano::bootstrap_attempts::bootstrap_attempts () :
	handle{ rsnano::rsn_bootstrap_attempts_create () }
{
}

nano::bootstrap_attempts::bootstrap_attempts (rsnano::BootstrapAttemptsHandle * handle) :
	handle{ handle }
{
}

nano::bootstrap_attempts::~bootstrap_attempts () noexcept
{
	rsnano::rsn_bootstrap_attempts_destroy (handle);
}

void nano::bootstrap_attempts::add (std::shared_ptr<nano::bootstrap_attempt> attempt_a)
{
	rsnano::rsn_bootstrap_attempts_add (handle, attempt_a->handle);
}

void nano::bootstrap_attempts::remove (uint64_t incremental_id_a)
{
	rsnano::rsn_bootstrap_attempts_remove (handle, incremental_id_a);
}

void nano::bootstrap_attempts::clear ()
{
	rsnano::rsn_bootstrap_attempts_clear (handle);
}

std::shared_ptr<nano::bootstrap_attempt> nano::bootstrap_attempts::find (uint64_t incremental_id_a)
{
	auto attempt_handle = rsnano::rsn_bootstrap_attempts_find (handle, incremental_id_a);
	return attempt_from_handle (attempt_handle);
}
std::size_t nano::bootstrap_attempts::size ()
{
	return rsnano::rsn_bootstrap_attempts_size (handle);
}
uint64_t nano::bootstrap_attempts::create_incremental_id ()
{
	return rsnano::rsn_bootstrap_attempts_incremental_id (handle);
}
uint64_t nano::bootstrap_attempts::total_attempts () const
{
	return rsnano::rsn_bootstrap_attempts_total_attempts (handle);
}

boost::property_tree::ptree nano::bootstrap_attempts::attempts_information ()
{
	boost::property_tree::ptree attempts;
	rsnano::rsn_bootstrap_attempts_attempts_information (handle, &attempts);
	return attempts;
}
