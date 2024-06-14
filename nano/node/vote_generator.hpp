#pragma once

#include <nano/lib/locks.hpp>
#include <nano/lib/logging.hpp>
#include <nano/lib/numbers.hpp>
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
	vote_generator (rsnano::VoteGeneratorHandle * handle);
	~vote_generator ();

	/** Queue items for vote generation, or broadcast votes already in cache */
	void add (nano::root const &, nano::block_hash const &);

	rsnano::VoteGeneratorHandle * handle;
};
}
