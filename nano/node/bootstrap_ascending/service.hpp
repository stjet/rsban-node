#pragma once

#include <nano/lib/locks.hpp>
#include <nano/lib/numbers.hpp>
#include <nano/lib/observer_set.hpp>
#include <nano/lib/timer.hpp>
#include <nano/node/bandwidth_limiter.hpp>
#include <nano/node/bootstrap/bootstrap_config.hpp>

namespace rsnano
{
class BootstrapAscendingHandle;
}
namespace nano
{
class block_processor;
class ledger;
class network;
class node_config;

namespace transport
{
	class channel;
}

namespace bootstrap_ascending
{
	class service
	{
	public:
		service (nano::node_config &, nano::block_processor &, nano::ledger &, nano::network &, nano::stats &);
		service (rsnano::BootstrapAscendingHandle * handle);
		service (service const &) = delete;
		~service ();

		void start ();
		void stop ();

		/**
		 * Process `asc_pull_ack` message coming from network
		 */
		void process (nano::asc_pull_ack const & message, std::shared_ptr<nano::transport::channel> channel);

		rsnano::BootstrapAscendingHandle * handle;
	};
}
}
