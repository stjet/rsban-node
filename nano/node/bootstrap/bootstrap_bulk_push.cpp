#include "nano/lib/rsnano.hpp"

#include <nano/node/bootstrap/bootstrap_attempt.hpp>
#include <nano/node/bootstrap/bootstrap_bulk_push.hpp>
#include <nano/node/bootstrap/bootstrap_legacy.hpp>
#include <nano/node/node.hpp>
#include <nano/node/transport/tcp.hpp>

#include <boost/format.hpp>

nano::bulk_push_client::bulk_push_client (std::shared_ptr<nano::node> const & node_a, std::shared_ptr<nano::bootstrap_client> const & connection_a, std::shared_ptr<nano::bootstrap_attempt_legacy> const & attempt_a) :
	node_weak (node_a),
	connection (connection_a),
	attempt (attempt_a)
{
}

nano::bulk_push_client::~bulk_push_client ()
{
}

void nano::bulk_push_client::start ()
{
	auto node = node_weak.lock ();
	if (!node)
	{
		return;
	}
	nano::bulk_push message{ node->network_params.network };
	auto this_l (shared_from_this ());
	connection->send (
	message, [this_l] (boost::system::error_code const & ec, std::size_t size_a) {
		auto node = this_l->node_weak.lock ();
		if (!node)
		{
			return;
		}
		if (!ec)
		{
			this_l->push ();
		}
		else
		{
			if (node->config->logging.bulk_pull_logging ())
			{
				node->logger->try_log (boost::str (boost::format ("Unable to send bulk_push request: %1%") % ec.message ()));
			}
		}
	},
	nano::transport::buffer_drop_policy::no_limiter_drop);
}

void nano::bulk_push_client::push ()
{
	auto node = node_weak.lock ();
	if (!node)
	{
		return;
	}
	std::shared_ptr<nano::block> block;
	bool finished (false);
	while (block == nullptr && !finished)
	{
		if (current_target.first.is_zero () || current_target.first == current_target.second)
		{
			finished = attempt->request_bulk_push_target (current_target);
		}
		if (!finished)
		{
			block = node->block (current_target.first);
			if (block == nullptr)
			{
				current_target.first = nano::block_hash (0);
			}
			else
			{
				if (node->config->logging.bulk_pull_logging ())
				{
					node->logger->try_log ("Bulk pushing range ", current_target.first.to_string (), " down to ", current_target.second.to_string ());
				}
			}
		}
	}
	if (finished)
	{
		send_finished ();
	}
	else
	{
		current_target.first = block->previous ();
		push_block (*block);
	}
}

void nano::bulk_push_client::send_finished ()
{
	nano::shared_const_buffer buffer (static_cast<uint8_t> (nano::block_type::not_a_block));
	auto this_l (shared_from_this ());
	connection->send_buffer (buffer, [this_l] (boost::system::error_code const & ec, std::size_t size_a) {
		try
		{
			this_l->promise.set_value (false);
		}
		catch (std::future_error &)
		{
		}
	});
}

void nano::bulk_push_client::push_block (nano::block const & block_a)
{
	std::vector<uint8_t> buffer;
	{
		nano::vectorstream stream (buffer);
		nano::serialize_block (stream, block_a);
	}
	auto this_l (shared_from_this ());
	connection->send_buffer (nano::shared_const_buffer (std::move (buffer)), [this_l] (boost::system::error_code const & ec, std::size_t size_a) {
		auto node = this_l->node_weak.lock ();
		if (!node)
		{
			return;
		}
		if (!ec)
		{
			this_l->push ();
		}
		else
		{
			if (node->config->logging.bulk_pull_logging ())
			{
				node->logger->try_log (boost::str (boost::format ("Error sending block during bulk push: %1%") % ec.message ()));
			}
		}
	});
}

nano::bulk_push_server::bulk_push_server (std::shared_ptr<nano::node> const & node_a, std::shared_ptr<nano::transport::tcp_server> const & connection_a) :
	handle{ rsnano::rsn_bulk_push_server_create (
	connection_a->handle,
	node_a->ledger.handle,
	nano::to_logger_handle (node_a->logger),
	node_a->bootstrap_workers->handle,
	node_a->config->logging.bulk_pull_logging (),
	node_a->config->logging.network_packet_logging (),
	node_a->block_processor.handle,
	node_a->bootstrap_initiator.handle,
	node_a->stats->handle,
	&node_a->config->network_params.work.dto) }
{
}

void nano::bulk_push_server::throttled_receive ()
{
	rsnano::rsn_bulk_push_server_throttled_receive (handle);
}
