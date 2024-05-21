#include "nano/lib/rsnano.hpp"

#include <nano/lib/blocks.hpp>
#include <nano/node/blockprocessor.hpp>
#include <nano/node/process_live_dispatcher.hpp>
#include <nano/node/scheduler/priority.hpp>
#include <nano/node/vote_cache.hpp>
#include <nano/node/websocket.hpp>
#include <nano/secure/common.hpp>
#include <nano/secure/ledger.hpp>
#include <nano/store/component.hpp>

nano::process_live_dispatcher::process_live_dispatcher (rsnano::ProcessLiveDispatcherHandle * handle) :
	handle{ handle}
{}

nano::process_live_dispatcher::~process_live_dispatcher ()
{
	rsnano::rsn_process_live_dispatcher_destroy (handle);
}

void nano::process_live_dispatcher::connect (nano::block_processor & block_processor)
{
	rsnano::rsn_process_live_dispatcher_connect (handle, block_processor.handle);
}
