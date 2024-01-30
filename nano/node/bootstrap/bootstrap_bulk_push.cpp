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
	auto attempt_l = attempt.lock ();
	if (!node || node->is_stopped () || !attempt_l)
	{
		return;
	}
	nano::bulk_push message{ node->network_params.network };
	auto this_l{ shared_from_this () };
	connection->send (
	message, [this_l] (boost::system::error_code const & ec, std::size_t size_a) {
		auto node = this_l->node_weak.lock ();
		if (!node || node->is_stopped ())
		{
			return;
		}
		if (!ec)
		{
			this_l->push ();
		}
		else
		{
			node->nlogger->debug (nano::log::type::bulk_push_client, "Unable to send bulk push request: {}", ec.message ());
		}
	},
	nano::transport::buffer_drop_policy::no_limiter_drop);
}

void nano::bulk_push_client::push ()
{
	auto node = node_weak.lock ();
	auto attempt_l = attempt.lock ();
	if (!node || node->is_stopped () || !attempt_l)
	{
		return;
	}
	std::shared_ptr<nano::block> block;
	bool finished (false);
	while (block == nullptr && !finished)
	{
		if (current_target.first.is_zero () || current_target.first == current_target.second)
		{
			finished = attempt_l->request_bulk_push_target (current_target);
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
				node->nlogger->debug (nano::log::type::bulk_push_client, "Bulk pushing range: [{}:{}]", current_target.first.to_string (), current_target.second.to_string ());
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
	std::weak_ptr<nano::bulk_push_client> this_w = (shared_from_this ());
	connection->send_buffer (nano::shared_const_buffer (std::move (buffer)), [this_w] (boost::system::error_code const & ec, std::size_t size_a) {
		auto this_l = this_w.lock ();
		if (!this_l)
			return;
		auto node = this_l->node_weak.lock ();
		if (!node || node->is_stopped ())
		{
			return;
		}
		if (!ec)
		{
			this_l->push ();
		}
		else
		{
			node->nlogger->debug (nano::log::type::bulk_push_client, "Error sending block during bulk push: {}", ec.message ());
		}
	});
}
