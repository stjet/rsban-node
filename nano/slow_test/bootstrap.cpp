#include <nano/lib/rpcconfig.hpp>
#include <nano/lib/thread_runner.hpp>
#include <nano/node/bootstrap/bootstrap_server.hpp>
#include <nano/node/ipc/ipc_server.hpp>
#include <nano/node/json_handler.hpp>
#include <nano/node/transport/transport.hpp>
#include <nano/rpc/rpc.hpp>
#include <nano/rpc/rpc_request_processor.hpp>
#include <nano/secure/ledger.hpp>
#include <nano/test_common/network.hpp>
#include <nano/test_common/system.hpp>
#include <nano/test_common/testutil.hpp>

#include <gtest/gtest.h>

#include <boost/format.hpp>

using namespace std::chrono_literals;

namespace
{
void wait_for_key ()
{
	int junk;
	std::cin >> junk;
}

class rpc_wrapper
{
public:
	rpc_wrapper (nano::test::system & system, nano::node & node, uint16_t port) :
		node_rpc_config{},
		rpc_config{ node.network_params.network, port, true },
		ipc{ node, node_rpc_config },
		ipc_rpc_processor{ system.async_rt.io_ctx, rpc_config },
		rpc{ system.async_rt.io_ctx, rpc_config, ipc_rpc_processor }
	{
	}

	void start ()
	{
		rpc.start ();
	}

public:
	nano::node_rpc_config node_rpc_config;
	nano::rpc_config rpc_config;
	nano::ipc::ipc_server ipc;
	nano::ipc_rpc_processor ipc_rpc_processor;
	nano::rpc rpc;
};

std::unique_ptr<rpc_wrapper> start_rpc (nano::test::system & system, nano::node & node, uint16_t port)
{
	auto rpc = std::make_unique<rpc_wrapper> (system, node, port);
	rpc->start ();
	return rpc;
}
}
