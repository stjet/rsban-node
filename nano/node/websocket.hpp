#pragma once

#include "nano/lib/rsnano.hpp"

#include <nano/boost/asio/strand.hpp>
#include <nano/boost/beast/core.hpp>
#include <nano/boost/beast/websocket.hpp>
#include <nano/lib/asio.hpp>
#include <nano/lib/numbers.hpp>
#include <nano/lib/work.hpp>
#include <nano/node/common.hpp>
#include <nano/node/vote_with_weight_info.hpp>
#include <nano/node/websocketconfig.hpp>
#include <nano/secure/common.hpp>

#include <boost/property_tree/json_parser.hpp>

#include <memory>
#include <string>
#include <vector>

namespace nano
{
class block;
class election_status;
enum class election_status_type : uint8_t;
class ledger;
class logger;
class node_observers;
class telemetry_data;
class vote;
class wallets;
}

namespace nano
{
namespace websocket
{
	class listener;
	class confirmation_options;
	class session;

	/** Supported topics */
	enum class topic
	{
		invalid = 0,
		/** Acknowledgement of prior incoming message */
		ack,
		/** A confirmation message */
		confirmation,
		/** Started election message*/
		started_election,
		/** Stopped election message (dropped elections due to bounding or block lost the elections) */
		stopped_election,
		/** A vote message **/
		vote,
		/** Work generation message */
		work,
		/** A bootstrap message */
		bootstrap,
		/** A telemetry message */
		telemetry,
		/** New block arrival message*/
		new_unconfirmed_block,
		/** Auxiliary length, not a valid topic, must be the last enum */
		_length
	};
	constexpr std::size_t number_topics{ static_cast<std::size_t> (topic::_length) - static_cast<std::size_t> (topic::invalid) };

	/** A message queued for broadcasting */
	class message final
	{
	public:
		message (nano::websocket::topic topic_a) :
			topic (topic_a)
		{
		}
		message (nano::websocket::topic topic_a, boost::property_tree::ptree & tree_a) :
			topic (topic_a), contents (tree_a)
		{
		}

		rsnano::MessageDto to_dto () const;

		std::string to_string () const;
		nano::websocket::topic topic;
		boost::property_tree::ptree contents;
	};

	/** Message builder. This is expanded with new builder functions are necessary. */
	class message_builder final
	{
	public:
		message block_confirmed (std::shared_ptr<nano::block> const & block_a, nano::account const & account_a, nano::amount const & amount_a, std::string subtype, bool include_block, nano::election_status const & election_status_a, std::vector<nano::vote_with_weight_info> const & election_votes_a, nano::websocket::confirmation_options const & options_a);
		message started_election (nano::block_hash const & hash_a);
		message stopped_election (nano::block_hash const & hash_a);
		message vote_received (std::shared_ptr<nano::vote> const & vote_a, nano::vote_code code_a);
		message work_generation (nano::work_version const version_a, nano::block_hash const & root_a, uint64_t const work_a, uint64_t const difficulty_a, uint64_t const publish_threshold_a, std::chrono::milliseconds const & duration_a, std::string const & peer_a, std::vector<std::string> const & bad_peers_a, bool const completed_a = true, bool const cancelled_a = false);
		message work_cancelled (nano::work_version const version_a, nano::block_hash const & root_a, uint64_t const difficulty_a, uint64_t const publish_threshold_a, std::chrono::milliseconds const & duration_a, std::vector<std::string> const & bad_peers_a);
		message work_failed (nano::work_version const version_a, nano::block_hash const & root_a, uint64_t const difficulty_a, uint64_t const publish_threshold_a, std::chrono::milliseconds const & duration_a, std::vector<std::string> const & bad_peers_a);
		message bootstrap_started (std::string const & id_a, std::string const & mode_a);
		message bootstrap_exited (std::string const & id_a, std::string const & mode_a, std::chrono::steady_clock::time_point const start_time_a, uint64_t const total_blocks_a);
		message telemetry_received (nano::telemetry_data const &, nano::endpoint const &);
		message new_block_arrived (nano::block const & block_a);
	};

	/** Options for subscriptions */
	class options
	{
	public:
		options ();
		options (rsnano::WebsocketOptionsHandle * handle);
		options (options const &) = delete;
		virtual ~options ();

	protected:
		/**
		 * Checks if a message should be filtered for default options (no options given).
		 * @param message_a the message to be checked
		 * @return false - the message should always be broadcasted
		 */
		virtual bool should_filter (message const & message_a) const
		{
			return false;
		}
		/**
		 * Update options, if available for a given topic
		 * @return false on success
		 */
		virtual bool update (boost::property_tree::ptree & options_a)
		{
			return true;
		}

		friend class session;

	public:
		rsnano::WebsocketOptionsHandle * handle;
	};

	/**
	 * Options for block confirmation subscriptions
	 * Non-filtering options:
	 * - "include_block" (bool, default true) - if false, do not include block contents. Only account, amount and hash will be included.
	 * Filtering options:
	 * - "all_local_accounts" (bool) - will only not filter blocks that have local wallet accounts as source/destination
	 * - "accounts" (array of std::strings) - will only not filter blocks that have these accounts as source/destination
	 * @remark Both options can be given, the resulting filter is an intersection of individual filters
	 * @warn Legacy blocks are always filtered (not broadcasted)
	 */
	class confirmation_options final : public options
	{
	public:
		confirmation_options (rsnano::WebsocketOptionsHandle * handle);
		confirmation_options (nano::wallets & wallets_a, nano::logger & logger_a);
		confirmation_options (boost::property_tree::ptree & options_a, nano::wallets & wallets_a, nano::logger & logger_a);

		/**
		 * Checks if a message should be filtered for given block confirmation options.
		 * @param message_a the message to be checked
		 * @return false if the message should be broadcasted, true if it should be filtered
		 */
		bool should_filter (message const & message_a) const override;

		/**
		 * Update some existing options
		 * Filtering options:
		 * - "accounts_add" (array of std::strings) - additional accounts for which blocks should not be filtered
		 * - "accounts_del" (array of std::strings) - accounts for which blocks should be filtered
		 * @return false
		 */
		bool update (boost::property_tree::ptree & options_a) override;

		/** Returns whether or not block contents should be included */
		bool get_include_block () const
		{
			return rsnano::rsn_confirmation_options_include_block (handle);
		}

		/** Returns whether or not to include election info, such as tally and duration */
		bool get_include_election_info () const
		{
			return rsnano::rsn_confirmation_options_include_election_info (handle);
		}

		/** Returns whether or not to include election info with votes */
		bool get_include_election_info_with_votes () const
		{
			return rsnano::rsn_confirmation_options_include_election_info_with_votes (handle);
		}

		/** Returns whether or not to include sideband info */
		bool get_include_sideband_info () const
		{
			return rsnano::rsn_confirmation_options_include_sideband_info (handle);
		}
	};

	/**
	 * Filtering options for vote subscriptions
	 * Possible filtering options:
	 * * "representatives" (array of std::strings) - will only broadcast votes from these representatives
	 */
	class vote_options final : public options
	{
	public:
		vote_options (boost::property_tree::ptree const & options_a, nano::logger & logger_a);

		/**
		 * Checks if a message should be filtered for given vote received options.
		 * @param message_a the message to be checked
		 * @return false if the message should be broadcasted, true if it should be filtered
		 */
		bool should_filter (message const & message_a) const override;
	};

	/** Creates a new session for each incoming connection */
	class listener final : public std::enable_shared_from_this<listener>
	{
	public:
		listener (rsnano::async_runtime & async_rt, nano::wallets & wallets_a, boost::asio::io_context & io_ctx_a, boost::asio::ip::tcp::endpoint endpoint_a);
		listener (listener const &) = delete;
		~listener ();

		/** Start accepting connections */
		void run ();

		/** Close all websocket sessions and stop listening for new connections */
		void stop ();

		/** Broadcast block confirmation. The content of the message depends on subscription options (such as "include_block") */
		void broadcast_confirmation (
		std::shared_ptr<nano::block> const & block_a,
		nano::account const & account_a,
		nano::amount const & amount_a,
		std::string const & subtype,
		nano::election_status const & election_status_a,
		std::vector<nano::vote_with_weight_info> const & election_votes_a);

		/** Broadcast \p message to all session subscribing to the message topic. */
		void broadcast (nano::websocket::message message_a);

		std::uint16_t listening_port ();

		/**
		 * Per-topic subscribers check. Relies on all sessions correctly increasing and
		 * decreasing the subscriber counts themselves.
		 */
		bool any_subscriber (nano::websocket::topic const & topic_a) const
		{
			return subscriber_count (topic_a) > 0;
		}
		/** Getter for subscriber count of a specific topic*/
		std::size_t subscriber_count (nano::websocket::topic const & topic_a) const;

		rsnano::WebsocketListenerHandle * handle;
	};
}

/**
 * Wrapper of websocket related functionality that node interacts with
 */
class websocket_server
{
public:
	websocket_server (rsnano::async_runtime & async_rt, nano::websocket::config &, nano::node_observers &, nano::wallets &, nano::ledger &, boost::asio::io_context &, nano::logger &);

	void start ();
	void stop ();

private: // Dependencies
	nano::websocket::config const & config;
	nano::node_observers & observers;
	nano::wallets & wallets;
	nano::ledger & ledger;
	boost::asio::io_context & io_ctx;
	nano::logger & logger;

public:
	// TODO: Encapsulate, this is public just because existing code needs it
	std::shared_ptr<nano::websocket::listener> server;
};
}
