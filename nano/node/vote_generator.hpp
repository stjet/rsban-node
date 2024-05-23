#pragma once

#include <nano/lib/locks.hpp>
#include <nano/lib/logging.hpp>
#include <nano/lib/numbers.hpp>
#include <nano/lib/processing_queue.hpp>
#include <nano/lib/utility.hpp>
#include <nano/node/wallet.hpp>
#include <nano/secure/common.hpp>

namespace rsnano
{
class VoteGeneratorHandle;
}

namespace nano
{
class ledger;
class local_vote_history;
class network;
class node;
class node_config;
class stats;
class vote_processor;
class vote_spacing;
class wallets;
class representative_register;
class vote_processor_queue;
}
namespace nano::transport
{
class channel;
}

namespace nano
{
class vote_generator final
{
public:
	vote_generator (nano::node & node_a, nano::node_config const & config_a, nano::ledger & ledger_a, nano::wallets & wallets_a, nano::vote_processor & vote_processor_a, nano::vote_processor_queue & vote_processor_queue_a, nano::local_vote_history & history_a, nano::network & network_a, nano::stats & stats_a, nano::representative_register & representative_register_a, bool is_final_a);
	vote_generator (rsnano::VoteGeneratorHandle * handle);
	~vote_generator ();

	/** Queue items for vote generation, or broadcast votes already in cache */
	void add (nano::root const &, nano::block_hash const &);

	void start ();
	void stop ();
	rsnano::VoteGeneratorHandle * handle;

	friend std::unique_ptr<container_info_component> collect_container_info (vote_generator & vote_generator, std::string const & name);
};

std::unique_ptr<container_info_component> collect_container_info (vote_generator & generator, std::string const & name);
}
