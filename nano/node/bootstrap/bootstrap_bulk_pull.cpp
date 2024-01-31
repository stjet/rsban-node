#include "nano/lib/blocks.hpp"
#include "nano/lib/epoch.hpp"
#include "nano/lib/logging.hpp"
#include "nano/lib/rsnano.hpp"
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

nano::bulk_pull_client::bulk_pull_client (std::shared_ptr<nano::node> const & node_a, std::shared_ptr<nano::bootstrap_client> const & connection_a, std::shared_ptr<nano::bootstrap_attempt> const & attempt_a, nano::pull_info const & pull_a) :
	node{ node_a },
	connections{ node_a->bootstrap_initiator.connections },
	connection{ connection_a },
	attempt{ attempt_a },
	pull{ pull_a },
	block_deserializer{ std::make_shared<nano::bootstrap::block_deserializer> (node_a->async_rt) },
	logger{ node_a->logger }
{
	attempt->notify_all ();
}

nano::bulk_pull_client::~bulk_pull_client ()
{
	auto node_l = node.lock ();
	if (!node_l)
	{
		return;
	}
	/* If received end block is not expected end block
	Or if given start and end blocks are from different chains (i.e. forked node or malicious node) */
	if (expected != pull.end && !expected.is_zero ())
	{
		pull.head = expected;
		if (attempt->get_mode () != nano::bootstrap_mode::legacy)
		{
			pull.account_or_head = expected;
		}
		pull.processed += pull_blocks - unexpected_count;
		node_l->bootstrap_initiator.connections->requeue_pull (pull, network_error);

		logger->debug (nano::log::type::bulk_pull_client, "Bulk pull end block is not expected {} for account {} or head block {}", pull.end.to_string (), pull.account_or_head.to_account (), pull.account_or_head.to_string ());
	}
	else
	{
		node_l->bootstrap_initiator.cache.remove (pull);
	}
	attempt->pull_finished ();
}

void nano::bulk_pull_client::request ()
{
	auto node_l = node.lock ();
	if (!node_l || node_l->is_stopped ())
	{
		return;
	}
	debug_assert (!pull.head.is_zero () || pull.retry_limit <= node_l->network_params.bootstrap.lazy_retry_limit);
	expected = pull.head;
	nano::bulk_pull::bulk_pull_payload payload;
	if (pull.head == pull.head_original && pull.attempts % 4 < 3)
	{
		// Account for new pulls
		payload.start = pull.account_or_head;
	}
	else
	{
		// Head for cached pulls or accounts with public key equal to existing block hash (25% of attempts)
		payload.start = pull.account_or_head;
	}
	payload.end = pull.end;
	payload.count = pull.count;
	payload.ascending = false;
	nano::bulk_pull req{ node_l->network_params.network, payload };

	if (attempt->should_log ())
	{
		logger->debug (nano::log::type::bulk_pull_client, "Accounts in pull queue: {}", attempt->get_pulling ());
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
			this_l->throttled_receive_block ();
		}
		else
		{
			this_l->logger->debug (nano::log::type::bulk_pull_client, "Error sending bulk pull request to: {} ({})", this_l->connection->channel_string (), ec.message ());
			node_l->stats->inc (nano::stat::type::bootstrap, nano::stat::detail::bulk_pull_request_failure, nano::stat::dir::in);
		}
	},
	nano::transport::buffer_drop_policy::no_limiter_drop);
}

void nano::bulk_pull_client::throttled_receive_block ()
{
	auto node_l = node.lock ();
	if (!node_l || node_l->is_stopped ())
	{
		return;
	}
	debug_assert (!network_error);
	if (!node_l->block_processor.half_full () && !node_l->block_processor.flushing ())
	{
		receive_block ();
	}
	else
	{
		auto this_l (shared_from_this ());
		node_l->workers->add_timed_task (std::chrono::steady_clock::now () + std::chrono::seconds (1), [this_l] () {
			if (!this_l->connection->get_pending_stop () && !this_l->attempt->get_stopped ())
			{
				this_l->throttled_receive_block ();
			}
		});
	}
}

void nano::bulk_pull_client::receive_block ()
{
	auto socket{ connection->get_socket () };
	block_deserializer->read (*socket, [this_l = shared_from_this ()] (boost::system::error_code ec, std::shared_ptr<nano::block> block) {
		this_l->received_block (ec, block);
	});
}

void nano::bulk_pull_client::received_block (boost::system::error_code ec, std::shared_ptr<nano::block> block)
{
	auto node_l = node.lock ();
	if (!node_l || node_l->is_stopped ())
	{
		return;
	}
	if (ec)
	{
		network_error = true;
		return;
	}
	if (block == nullptr)
	{
		// Avoid re-using slow peers, or peers that sent the wrong blocks.
		if (!connection->get_pending_stop () && (expected == pull.end || (pull.count != 0 && pull.count == pull_blocks)))
		{
			connections->pool_connection (connection);
		}
		return;
	}
	if (node_l->network_params.work.validate_entry (*block))
	{
		logger->debug (nano::log::type::bulk_pull_client, "Insufficient work for bulk pull block: {}", block->hash ().to_string ());
		node_l->stats->inc_detail_only (nano::stat::type::error, nano::stat::detail::insufficient_work);
		return;
	}
	auto hash = block->hash ();
	// Is block expected?
	bool block_expected (false);
	// Unconfirmed head is used only for lazy destinations if legacy bootstrap is not available, see nano::bootstrap_attempt::lazy_destinations_increment (...)
	bool unconfirmed_account_head (node_l->flags.disable_legacy_bootstrap () && pull_blocks == 0 && pull.retry_limit <= node_l->network_params.bootstrap.lazy_retry_limit && expected == pull.account_or_head.as_block_hash () && block->account () == pull.account_or_head.as_account ());
	if (hash == expected || unconfirmed_account_head)
	{
		expected = block->previous ();
		block_expected = true;
	}
	else
	{
		unexpected_count++;
	}
	if (pull_blocks == 0 && block_expected)
	{
		known_account = block->account ();
	}
	if (connection->inc_block_count () == 0)
	{
		connection->set_start_time ();
	}
	attempt->total_blocks_inc ();
	pull_blocks++;
	bool stop_pull (attempt->process_block (block, known_account, pull_blocks, pull.count, block_expected, pull.retry_limit));
	if (!stop_pull && !connection->get_hard_stop ())
	{
		/* Process block in lazy pull if not stopped
		Stop usual pull request with unexpected block & more than 16k blocks processed
		to prevent spam */
		if (attempt->get_mode () != nano::bootstrap_mode::legacy || unexpected_count < 16384)
		{
			throttled_receive_block ();
		}
	}
	else if (!stop_pull && block_expected)
	{
		connections->pool_connection (connection);
	}
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
	auto logger_handle{nano::to_logger_handle(node_a->logger)};

	handle = rsnano::rsn_bulk_pull_server_create (
			request_a->handle, 
			connection_a->handle, 
			node_a->ledger.handle, 
			logger_handle.handle, 
			node_a->bootstrap_workers->handle);
}

nano::bulk_pull_server::~bulk_pull_server ()
{
	rsnano::rsn_bulk_pull_server_destroy (handle);
}

nano::bulk_pull_account_server::bulk_pull_account_server (std::shared_ptr<nano::node> const & node_a, std::shared_ptr<nano::transport::tcp_server> const & connection_a, std::unique_ptr<nano::bulk_pull_account> request_a) 
{
	auto logger_handle{nano::to_logger_handle(node_a->logger)};
	handle = rsnano::rsn_bulk_pull_account_server_create (
			request_a->handle, 
			connection_a->handle, 
			node_a->ledger.handle, 
			logger_handle.handle, 
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
