#pragma once
namespace rsnano
{
class ProcessLiveDispatcherHandle;
}

namespace nano::store
{
class transaction;
}

namespace nano
{
class ledger;
class vote_cache;
class websocket_server;
class block_processor;
class process_return;
class block;

namespace scheduler
{
	class priority;
}

// Observes confirmed blocks and dispatches the process_live function.
class process_live_dispatcher
{
public:
	process_live_dispatcher (nano::ledger &, nano::scheduler::priority &, nano::vote_cache &, nano::websocket_server &);
	process_live_dispatcher (process_live_dispatcher const &) = delete;
	~process_live_dispatcher ();
	void connect (nano::block_processor & block_processor);

	rsnano::ProcessLiveDispatcherHandle * handle;
};
}
