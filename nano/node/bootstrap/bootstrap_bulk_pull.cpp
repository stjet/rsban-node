#include "nano/lib/blocks.hpp"
#include "nano/lib/epoch.hpp"
#include "nano/lib/logging.hpp"
#include "nano/lib/rsnano.hpp"
#include "nano/lib/rsnanoutils.hpp"
#include "nano/lib/threading.hpp"
#include "nano/node/messages.hpp"
#include "nano/secure/common.hpp"

#include <nano/node/bootstrap/block_deserializer.hpp>
#include <nano/node/bootstrap/bootstrap.hpp>
#include <nano/node/bootstrap/bootstrap_bulk_pull.hpp>
#include <nano/node/bootstrap/bootstrap_connections.hpp>
#include <nano/node/bootstrap/bootstrap_lazy.hpp>
#include <nano/node/node.hpp>
#include <nano/node/transport/tcp.hpp>
#include <nano/secure/ledger.hpp>

#include <boost/format.hpp>

nano::pull_info::pull_info (nano::hash_or_account const & account_or_head_a, nano::block_hash const & head_a, nano::block_hash const & end_a, uint64_t bootstrap_id_a, count_t count_a, unsigned retry_limit_a) :
	account_or_head (account_or_head_a),
	head (head_a),
	head_original (head_a),
	end (end_a),
	count (count_a),
	retry_limit (retry_limit_a),
	bootstrap_id (bootstrap_id_a)
{
}
rsnano::PullInfoDto nano::pull_info::to_dto () const
{
	rsnano::PullInfoDto dto;
	std::copy (std::begin (account_or_head.bytes), std::end (account_or_head.bytes), std::begin (dto.account_or_head));
	std::copy (std::begin (head.bytes), std::end (head.bytes), std::begin (dto.head));
	std::copy (std::begin (head_original.bytes), std::end (head_original.bytes), std::begin (dto.head_original));
	std::copy (std::begin (end.bytes), std::end (end.bytes), std::begin (dto.end));
	dto.count = count;
	dto.attempts = attempts;
	dto.processed = processed;
	dto.retry_limit = retry_limit;
	dto.bootstrap_id = bootstrap_id;
	return dto;
}

void nano::pull_info::load_dto (rsnano::PullInfoDto const & dto)
{
	std::copy (std::begin (dto.account_or_head), std::end (dto.account_or_head), std::begin (account_or_head.bytes));
	std::copy (std::begin (dto.head), std::end (dto.head), std::begin (head.bytes));
	std::copy (std::begin (dto.head_original), std::end (dto.head_original), std::begin (head_original.bytes));
	std::copy (std::begin (dto.end), std::end (dto.end), std::begin (end.bytes));
	count = dto.count;
	attempts = dto.attempts;
	processed = dto.processed;
	retry_limit = dto.retry_limit;
	bootstrap_id = dto.bootstrap_id;
}

nano::bulk_pull_client::bulk_pull_client (
std::shared_ptr<nano::node> const & node_a,
std::shared_ptr<nano::bootstrap_client> const & connection_a,
std::shared_ptr<nano::bootstrap_attempt> const & attempt_a,
nano::pull_info const & pull_a)
{
	auto nw_params{ node_a->network_params.to_dto () };
	auto pull_dto{ pull_a.to_dto () };
	handle = rsnano::rsn_bulk_pull_client_create (
	&nw_params,
	node_a->flags.handle,
	node_a->stats->handle,
	node_a->block_processor.handle,
	connection_a->handle,
	attempt_a->handle,
	node_a->workers->handle,
	node_a->async_rt.handle,
	node_a->bootstrap_initiator.connections->handle,
	node_a->bootstrap_initiator.handle,
	&pull_dto);
}

nano::bulk_pull_client::~bulk_pull_client ()
{
	rsnano::rsn_bulk_pull_client_destroy (handle);
}

void nano::bulk_pull_client::request ()
{
	rsnano::rsn_bulk_pull_client_request (handle);
}

nano::bulk_pull_account_client::bulk_pull_account_client (std::shared_ptr<nano::node> const & node_a, std::shared_ptr<nano::bootstrap_client> const & connection_a, std::shared_ptr<nano::bootstrap_attempt_wallet> const & attempt_a, nano::account const & account_a) :
	connection (connection_a),
	attempt (attempt_a),
	account (account_a),
	pull_blocks (0),
	node{ node_a }
{
	attempt->notify_all ();
}

nano::bulk_pull_account_client::~bulk_pull_account_client ()
{
	attempt->pull_finished ();
}

void nano::bulk_pull_account_client::request ()
{
	auto node_l = node.lock ();
	if (!node_l || node_l->is_stopped ())
	{
		return;
	}
	nano::bulk_pull_account::payload payload{};
	payload.account = account;
	payload.minimum_amount = node_l->config->receive_minimum;
	payload.flags = nano::bulk_pull_account_flags::pending_hash_and_amount;
	nano::bulk_pull_account req{ node_l->network_params.network, payload };

	node_l->logger->trace (nano::log::type::bulk_pull_account_client, nano::log::detail::requesting_pending,
	nano::log::arg{ "account", req.get_account ().to_account () }, // TODO: Convert to lazy eval
	nano::log::arg{ "connection", connection->channel_string () });

	if (attempt->should_log ())
	{
		node_l->logger->debug (nano::log::type::bulk_pull_account_client, "Accounts in pull queue: {}", attempt->wallet_size ());
	}

	auto this_l (shared_from_this ());
	connection->send (
	req, [this_l] (boost::system::error_code const & ec, std::size_t size_a) {
		auto node_l = this_l->node.lock ();
		if (!node_l || node_l->is_stopped ())
		{
			return;
		}
		if (!ec)
		{
			this_l->receive_pending ();
		}
		else
		{
			node_l->logger->debug (nano::log::type::bulk_pull_account_client, "Error starting bulk pull request to: {} ({})", this_l->connection->channel_string (), ec.message ());
			node_l->stats->inc (nano::stat::type::bootstrap, nano::stat::detail::bulk_pull_error_starting_request, nano::stat::dir::in);

			this_l->attempt->requeue_pending (this_l->account);
		}
	},
	nano::transport::buffer_drop_policy::no_limiter_drop);
}

void nano::bulk_pull_account_client::receive_pending ()
{
	auto this_l (shared_from_this ());
	std::size_t size_l (sizeof (nano::uint256_union) + sizeof (nano::uint128_union));
	connection->async_read (size_l, [this_l, size_l] (boost::system::error_code const & ec, std::size_t size_a) {
		auto node_l = this_l->node.lock ();
		if (!node_l || node_l->is_stopped ())
		{
			return;
		}
		// An issue with asio is that sometimes, instead of reporting a bad file descriptor during disconnect,
		// we simply get a size of 0.
		if (size_a == size_l)
		{
			if (!ec)
			{
				nano::block_hash pending;
				nano::bufferstream frontier_stream (this_l->connection->get_receive_buffer (), sizeof (nano::uint256_union));
				auto error1 (nano::try_read (frontier_stream, pending));
				(void)error1;
				debug_assert (!error1);
				nano::amount balance;
				nano::bufferstream balance_stream (this_l->connection->get_receive_buffer () + sizeof (nano::uint256_union), sizeof (nano::uint128_union));
				auto error2 (nano::try_read (balance_stream, balance));
				(void)error2;
				debug_assert (!error2);
				if (this_l->pull_blocks == 0 || !pending.is_zero ())
				{
					if (this_l->pull_blocks == 0 || balance.number () >= node_l->config->receive_minimum.number ())
					{
						this_l->pull_blocks++;
						{
							if (!pending.is_zero ())
							{
								if (!node_l->ledger.block_or_pruned_exists (pending))
								{
									node_l->bootstrap_initiator.bootstrap_lazy (pending, false);
								}
							}
						}
						this_l->receive_pending ();
					}
					else
					{
						this_l->attempt->requeue_pending (this_l->account);
					}
				}
				else
				{
					node_l->bootstrap_initiator.connections->pool_connection (this_l->connection);
				}
			}
			else
			{
				node_l->logger->debug (nano::log::type::bulk_pull_account_client, "Error while receiving bulk pull account frontier: {}", ec.message ());

				this_l->attempt->requeue_pending (this_l->account);
			}
		}
		else
		{
			node_l->logger->debug (nano::log::type::bulk_pull_account_client, "Invalid size: Expected {}, got: {}", size_l, size_a);

			this_l->attempt->requeue_pending (this_l->account);
		}
	});
}

void nano::bulk_pull_server::send_next ()
{
	rsnano::rsn_bulk_pull_server_send_next (handle);
}

nano::bulk_pull::count_t nano::bulk_pull_server::get_sent_count () const
{
	return rsnano::rsn_bulk_pull_server_sent_count (handle);
}

nano::bulk_pull::count_t nano::bulk_pull_server::get_max_count () const
{
	return rsnano::rsn_bulk_pull_server_max_count (handle);
}

nano::bulk_pull nano::bulk_pull_server::get_request () const
{
	return nano::bulk_pull{ rsnano::rsn_bulk_pull_server_request (handle) };
}

nano::block_hash nano::bulk_pull_server::get_current () const
{
	nano::block_hash current;
	rsnano::rsn_bulk_pull_server_current (handle, current.bytes.data ());
	return current;
}

std::shared_ptr<nano::block> nano::bulk_pull_server::get_next ()
{
	auto block_handle = rsnano::rsn_bulk_pull_server_get_next (handle);
	return nano::block_handle_to_block (block_handle);
}

nano::bulk_pull_server::bulk_pull_server (std::shared_ptr<nano::node> const & node_a, std::shared_ptr<nano::transport::tcp_server> const & connection_a, std::unique_ptr<nano::bulk_pull> request_a)
{
	handle = rsnano::rsn_bulk_pull_server_create (
	request_a->handle,
	connection_a->handle,
	node_a->ledger.handle,
	node_a->bootstrap_workers->handle);
}

nano::bulk_pull_server::~bulk_pull_server ()
{
	rsnano::rsn_bulk_pull_server_destroy (handle);
}

nano::bulk_pull_account_server::bulk_pull_account_server (std::shared_ptr<nano::node> const & node_a, std::shared_ptr<nano::transport::tcp_server> const & connection_a, std::unique_ptr<nano::bulk_pull_account> request_a)
{
	handle = rsnano::rsn_bulk_pull_account_server_create (
	request_a->handle,
	connection_a->handle,
	node_a->ledger.handle,
	node_a->bootstrap_workers->handle);
}

nano::bulk_pull_account_server::~bulk_pull_account_server ()
{
	rsnano::rsn_bulk_pull_account_server_destroy (handle);
}

void nano::bulk_pull_account_server::send_frontier ()
{
	rsnano::rsn_bulk_pull_account_server_send_frontier (handle);
}

std::pair<std::unique_ptr<nano::pending_key>, std::unique_ptr<nano::pending_info>> nano::bulk_pull_account_server::get_next ()
{
	std::pair<std::unique_ptr<nano::pending_key>, std::unique_ptr<nano::pending_info>> result;

	rsnano::PendingKeyDto key_dto;
	rsnano::PendingInfoDto info_dto;
	if (rsnano::rsn_bulk_pull_account_server_get_next (handle, &key_dto, &info_dto))
	{
		nano::account acc{};
		std::copy (std::begin (key_dto.account), std::end (key_dto.account), acc.bytes.data ());
		auto hash = nano::block_hash::from_bytes (&key_dto.hash[0]);
		result.first = std::make_unique<nano::pending_key> (acc, hash);

		nano::account source{};
		std::copy (std::begin (info_dto.source), std::end (info_dto.source), source.bytes.data ());
		nano::amount amount{};
		std::copy (std::begin (info_dto.amount), std::end (info_dto.amount), amount.bytes.data ());
		auto epoch = static_cast<nano::epoch> (info_dto.epoch);

		result.second = std::make_unique<nano::pending_info> (source, amount, epoch);
	}
	else
	{
		result.first = nullptr;
		result.second = nullptr;
	}

	return result;
}

nano::pending_key nano::bulk_pull_account_server::current_key ()
{
	rsnano::PendingKeyDto key_dto;
	rsnano::rsn_bulk_pull_account_server_current_key (handle, &key_dto);

	nano::account acc{};
	std::copy (std::begin (key_dto.account), std::end (key_dto.account), acc.bytes.data ());
	auto hash = nano::block_hash::from_bytes (&key_dto.hash[0]);
	return nano::pending_key{ acc, hash };
}

bool nano::bulk_pull_account_server::pending_address_only ()
{
	return rsnano::rsn_bulk_pull_account_server_pending_address_only (handle);
}

bool nano::bulk_pull_account_server::pending_include_address ()
{
	return rsnano::rsn_bulk_pull_account_server_pending_include_address (handle);
}

bool nano::bulk_pull_account_server::invalid_request ()
{
	return rsnano::rsn_bulk_pull_account_server_invalid_request (handle);
}
