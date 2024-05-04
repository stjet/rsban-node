#include "boost/asio/bind_executor.hpp"
#include "nano/lib/rsnano.hpp"

#include <nano/boost/asio/bind_executor.hpp>
#include <nano/boost/asio/dispatch.hpp>
#include <nano/boost/asio/strand.hpp>
#include <nano/lib/blocks.hpp>
#include <nano/lib/logging.hpp>
#include <nano/lib/rsnanoutils.hpp>
#include <nano/lib/work.hpp>
#include <nano/node/election_status.hpp>
#include <nano/node/node_observers.hpp>
#include <nano/node/transport/channel.hpp>
#include <nano/node/transport/transport.hpp>
#include <nano/node/wallet.hpp>
#include <nano/node/websocket.hpp>
#include <nano/secure/ledger.hpp>

#include <boost/algorithm/string.hpp>
#include <boost/property_tree/json_parser.hpp>

#include <algorithm>
#include <chrono>

nano::websocket::options::options () :
	handle{ rsnano::rsn_websocket_options_create () }
{
}
nano::websocket::options::options (rsnano::WebsocketOptionsHandle * handle) :
	handle{ handle }
{
}

nano::websocket::options::~options ()
{
	rsnano::rsn_websocket_options_destroy (handle);
}

nano::websocket::confirmation_options::confirmation_options (rsnano::WebsocketOptionsHandle * handle) :
	options (handle)
{
}

nano::websocket::confirmation_options::confirmation_options (nano::wallets & wallets_a, nano::logger & logger_a) :
	options (rsnano::rsn_confirmation_options_create (wallets_a.rust_handle, nullptr))
{
}

nano::websocket::confirmation_options::confirmation_options (boost::property_tree::ptree & options_a, nano::wallets & wallets_a, nano::logger & logger_a) :
	options (rsnano::rsn_confirmation_options_create (wallets_a.rust_handle, (void *)&options_a))
{
}

bool nano::websocket::confirmation_options::should_filter (nano::websocket::message const & message_a) const
{
	auto message_dto{ message_a.to_dto () };
	return rsnano::rsn_confirmation_options_should_filter (handle, &message_dto);
}

bool nano::websocket::confirmation_options::update (boost::property_tree::ptree & options_a)
{
	return rsnano::rsn_confirmation_options_update (handle, &options_a);
}

nano::websocket::vote_options::vote_options (boost::property_tree::ptree const & options_a, nano::logger & logger_a) :
	options (rsnano::rsn_vote_options_create (&const_cast<boost::property_tree::ptree &> (options_a)))
{
}

bool nano::websocket::vote_options::should_filter (nano::websocket::message const & message_a) const
{
	auto message_dto{ message_a.to_dto () };
	return rsnano::rsn_vote_options_should_filter (handle, &message_dto);
}

namespace
{
nano::websocket::topic to_topic (std::string const & topic_a)
{
	return static_cast<nano::websocket::topic> (rsnano::rsn_to_topic (topic_a.c_str ()));
}

std::string from_topic (nano::websocket::topic topic_a)
{
	rsnano::StringDto result;
	rsnano::rsn_from_topic (static_cast<uint8_t> (topic_a), &result);
	return rsnano::convert_dto_to_string (result);
}

class listener_subscriptions_lock
{
public:
	listener_subscriptions_lock (nano::websocket::session const & session) :
		handle{ rsnano::rsn_websocket_session_lock_subscriptions (session.handle) }
	{
	}

	listener_subscriptions_lock (listener_subscriptions_lock const &) = delete;

	~listener_subscriptions_lock ()
	{
		unlock ();
	}

	void unlock ()
	{
		if (handle != nullptr)
		{
			rsnano::rsn_listener_subscriptions_lock_destroy (handle);
			handle = nullptr;
		}
	}

	std::vector<nano::websocket::topic> topics () const
	{
		auto topics_handle = rsnano::rsn_listener_subscriptions_lock_get_topics (handle);
		auto len = rsnano::rsn_topic_vec_len (topics_handle);
		std::vector<nano::websocket::topic> result;
		result.reserve (len);
		for (auto i = 0; i < len; ++i)
		{
			auto topic = static_cast<nano::websocket::topic> (rsnano::rsn_topic_vec_get (topics_handle, i));
			result.push_back (topic);
		}
		rsnano::rsn_topic_vec_destroy (topics_handle);
		return result;
	}

	bool should_filter (nano::websocket::message const & message)
	{
		auto dto{ message.to_dto () };
		return rsnano::rsn_listener_subscriptions_lock_should_filter (handle, &dto);
	}

	bool set_options (nano::websocket::topic topic, nano::websocket::options const & options, socket_type::endpoint_type const & remote)
	{
		auto remote_dto{ rsnano::endpoint_to_dto (remote) };
		return rsnano::rsn_listener_subscriptions_lock_set_options (handle, static_cast<uint8_t> (topic), options.handle, &remote_dto);
	}

	bool update (nano::websocket::topic topic, boost::property_tree::ptree & message)
	{
		return rsnano::rsn_listener_subscriptions_lock_update (handle, static_cast<uint8_t> (topic), &message);
	}

	bool erase (nano::websocket::topic topic)
	{
		return rsnano::rsn_listener_subscriptions_lock_remove (handle, static_cast<uint8_t> (topic));
	}

	bool contains_topic (nano::websocket::topic topic)
	{
		return rsnano::rsn_listener_subscriptions_lock_contains_topic (handle, static_cast<uint8_t> (topic));
	}

	nano::websocket::confirmation_options get_confirmation_options (nano::websocket::topic topic, nano::wallets const & wallets)
	{
		auto opts_handle = rsnano::rsn_listener_subscriptions_lock_get_conf_opts_or_default (handle, static_cast<uint8_t> (topic), wallets.rust_handle);
		return { opts_handle };
	}

	rsnano::ListenerSubscriptionsLock * handle;
};
}

nano::websocket::session::session (nano::websocket::listener & listener_a, socket_type socket_a, nano::logger & logger_a) :
	ws_listener (listener_a),
	ws (std::move (socket_a)),
	strand{ ws.get_executor () },
	logger{ logger_a },
	handle{ rsnano::rsn_websocket_session_create () }
{
	{
		// Best effort attempt to get endpoint addresses
		boost::system::error_code ec;
		remote = ws.next_layer ().remote_endpoint (ec);
		debug_assert (!ec);
		local = ws.next_layer ().local_endpoint (ec);
		debug_assert (!ec);
	}

	logger.info (nano::log::type::websocket, "Session started ({})", nano::util::to_str (remote));
}

nano::websocket::session::~session ()
{
	{
		listener_subscriptions_lock subs_lock{ *this };
		for (auto topic : subs_lock.topics ())
		{
			ws_listener.decrease_subscriber_count (topic);
		}
	}
	rsnano::rsn_websocket_session_destroy (handle);
}

void nano::websocket::session::handshake ()
{
	auto this_l (shared_from_this ());
	// Websocket handshake
	ws.async_accept ([this_l] (boost::system::error_code const & ec) {
		if (!ec)
		{
			// Start reading incoming messages
			this_l->read ();
		}
		else
		{
			this_l->logger.error (nano::log::type::websocket, "Handshake failed: {} ({})", ec.message (), nano::util::to_str (this_l->remote));
		}
	});
}

void nano::websocket::session::close ()
{
	logger.info (nano::log::type::websocket, "Session closing ({})", nano::util::to_str (remote));

	auto this_l (shared_from_this ());
	boost::asio::dispatch (strand,
	[this_l] () {
		boost::beast::websocket::close_reason reason;
		reason.code = boost::beast::websocket::close_code::normal;
		reason.reason = "Shutting down";
		boost::system::error_code ec_ignore;
		this_l->ws.close (reason, ec_ignore);
	});
}

void nano::websocket::session::write (nano::websocket::message message_a)
{
	listener_subscriptions_lock subs_lock{ *this };
	if (message_a.topic == nano::websocket::topic::ack || !subs_lock.should_filter (message_a))
	{
		subs_lock.unlock ();
		auto this_l (shared_from_this ());
		boost::asio::post (strand,
		[message_a, this_l] () {
			bool write_in_progress = !this_l->send_queue.empty ();
			this_l->send_queue.emplace_back (message_a);
			if (!write_in_progress)
			{
				this_l->write_queued_messages ();
			}
		});
	}
}

void nano::websocket::session::write_queued_messages ()
{
	auto msg (send_queue.front ().to_string ());
	auto this_l (shared_from_this ());

	ws.async_write (nano::shared_const_buffer (msg),
	boost::asio::bind_executor (strand,
	[this_l] (boost::system::error_code ec, std::size_t bytes_transferred) {
		this_l->send_queue.pop_front ();
		if (!ec)
		{
			if (!this_l->send_queue.empty ())
			{
				this_l->write_queued_messages ();
			}
		}
	}));
}

void nano::websocket::session::read ()
{
	auto this_l (shared_from_this ());

	boost::asio::post (strand, [this_l] () {
		this_l->ws.async_read (this_l->read_buffer,
		boost::asio::bind_executor (this_l->strand,
		[this_l] (boost::system::error_code ec, std::size_t bytes_transferred) {
			if (!ec)
			{
				std::stringstream os;
				os << beast_buffers (this_l->read_buffer.data ());
				std::string incoming_message = os.str ();

				// Prepare next read by clearing the multibuffer
				this_l->read_buffer.consume (this_l->read_buffer.size ());

				boost::property_tree::ptree tree_msg;
				try
				{
					boost::property_tree::read_json (os, tree_msg);
					this_l->handle_message (tree_msg);
					this_l->read ();
				}
				catch (boost::property_tree::json_parser::json_parser_error const & ex)
				{
					this_l->logger.error (nano::log::type::websocket, "JSON parsing failed: {} ({})", ex.what (), nano::util::to_str (this_l->remote));
				}
			}
			else if (ec != boost::asio::error::eof)
			{
				this_l->logger.error (nano::log::type::websocket, "Read failed: {} ({})", ec.message (), nano::util::to_str (this_l->remote));
			}
		}));
	});
}

void nano::websocket::session::send_ack (std::string action_a, std::string id_a)
{
	nano::websocket::message msg (nano::websocket::topic::ack);
	boost::property_tree::ptree & message_l = msg.contents;
	message_l.add ("ack", action_a);
	message_l.add ("time", std::to_string (nano::milliseconds_since_epoch ()));
	if (!id_a.empty ())
	{
		message_l.add ("id", id_a);
	}
	write (msg);
}

void nano::websocket::session::handle_message (boost::property_tree::ptree const & message_a)
{
	std::string action (message_a.get<std::string> ("action", ""));
	auto topic_l (to_topic (message_a.get<std::string> ("topic", "")));
	auto ack_l (message_a.get<bool> ("ack", false));
	auto id_l (message_a.get<std::string> ("id", ""));
	auto action_succeeded (false);
	if (action == "subscribe" && topic_l != nano::websocket::topic::invalid)
	{
		auto options_text_l (message_a.get_child_optional ("options"));
		listener_subscriptions_lock subs_lock{ *this };
		std::unique_ptr<nano::websocket::options> options_l{ nullptr };
		if (options_text_l && topic_l == nano::websocket::topic::confirmation)
		{
			options_l = std::make_unique<nano::websocket::confirmation_options> (const_cast<boost::property_tree::ptree &> (options_text_l.get ()), ws_listener.get_wallets (), logger);
		}
		else if (options_text_l && topic_l == nano::websocket::topic::vote)
		{
			options_l = std::make_unique<nano::websocket::vote_options> (options_text_l.get (), logger);
		}
		else
		{
			options_l = std::make_unique<nano::websocket::options> ();
		}

		auto inserted = subs_lock.set_options (topic_l, *options_l, remote);
		if (inserted)
		{
			ws_listener.increase_subscriber_count (topic_l);
		}
		action_succeeded = true;
	}
	else if (action == "update")
	{
		listener_subscriptions_lock subs_lock{ *this };
		if (subs_lock.update (topic_l, const_cast<boost::property_tree::ptree &> (message_a)))
		{
			action_succeeded = true;
		}
	}
	else if (action == "unsubscribe" && topic_l != nano::websocket::topic::invalid)
	{
		listener_subscriptions_lock subs_lock{ *this };
		if (subs_lock.erase (topic_l))
		{
			logger.info (nano::log::type::websocket, "Removed subscription to topic: {} ({})", from_topic (topic_l), nano::util::to_str (remote));
			ws_listener.decrease_subscriber_count (topic_l);
		}
		action_succeeded = true;
	}
	else if (action == "ping")
	{
		action_succeeded = true;
		ack_l = "true";
		action = "pong";
	}
	if (ack_l && action_succeeded)
	{
		send_ack (action, id_l);
	}
}

void nano::websocket::listener::stop ()
{
	stopped = true;
	acceptor.close ();

	nano::lock_guard<nano::mutex> lk (sessions_mutex);
	for (auto & weak_session : sessions)
	{
		auto session_ptr (weak_session.lock ());
		if (session_ptr)
		{
			session_ptr->close ();
		}
	}
	sessions.clear ();
}

nano::websocket::listener::listener (nano::logger & logger_a, nano::wallets & wallets_a, boost::asio::io_context & io_ctx_a, boost::asio::ip::tcp::endpoint endpoint_a) :
	logger (logger_a),
	wallets (wallets_a),
	acceptor (io_ctx_a),
	socket (io_ctx_a)
{
	try
	{
		for (std::atomic<std::size_t> & item : topic_subscriber_count)
		{
			item = std::size_t (0);
		}
		acceptor.open (endpoint_a.protocol ());
		acceptor.set_option (boost::asio::socket_base::reuse_address (true));
		acceptor.bind (endpoint_a);
		acceptor.listen (boost::asio::socket_base::max_listen_connections);
	}
	catch (std::exception const & ex)
	{
		logger.error (nano::log::type::websocket, "Listen failed: {}", ex.what ());
	}
}

void nano::websocket::listener::run ()
{
	if (acceptor.is_open ())
	{
		accept ();
	}
}

void nano::websocket::listener::accept ()
{
	auto this_l (shared_from_this ());
	acceptor.async_accept (socket,
	[this_l] (boost::system::error_code const & ec) {
		this_l->on_accept (ec);
	});
}

void nano::websocket::listener::on_accept (boost::system::error_code ec)
{
	if (ec)
	{
		logger.error (nano::log::type::websocket, "Accept failed: {}", ec.message ());
	}
	else
	{
		// Create the session and initiate websocket handshake
		std::shared_ptr<nano::websocket::session> session;
		session = std::make_shared<nano::websocket::session> (*this, std::move (socket), logger);

		sessions_mutex.lock ();
		sessions.push_back (session);
		// Clean up expired sessions
		sessions.erase (std::remove_if (sessions.begin (), sessions.end (), [] (auto & elem) { return elem.expired (); }), sessions.end ());
		sessions_mutex.unlock ();
		session->handshake ();
	}

	if (!stopped)
	{
		accept ();
	}
}

void nano::websocket::listener::broadcast_confirmation (std::shared_ptr<nano::block> const & block_a, nano::account const & account_a, nano::amount const & amount_a, std::string const & subtype, nano::election_status const & election_status_a, std::vector<nano::vote_with_weight_info> const & election_votes_a)
{
	nano::websocket::message_builder builder;

	nano::lock_guard<nano::mutex> lk (sessions_mutex);
	boost::optional<nano::websocket::message> msg_with_block;
	boost::optional<nano::websocket::message> msg_without_block;
	for (auto & weak_session : sessions)
	{
		auto session_ptr (weak_session.lock ());
		if (session_ptr)
		{
			listener_subscriptions_lock subs_lock{ *session_ptr };
			if (subs_lock.contains_topic (nano::websocket::topic::confirmation))
			{
				auto conf_options{ subs_lock.get_confirmation_options (nano::websocket::topic::confirmation, wallets) };
				auto include_block = conf_options.get_include_block ();

				if (include_block && !msg_with_block)
				{
					msg_with_block = builder.block_confirmed (block_a, account_a, amount_a, subtype, include_block, election_status_a, election_votes_a, conf_options);
				}
				else if (!include_block && !msg_without_block)
				{
					msg_without_block = builder.block_confirmed (block_a, account_a, amount_a, subtype, include_block, election_status_a, election_votes_a, conf_options);
				}
				subs_lock.unlock ();

				session_ptr->write (include_block ? msg_with_block.get () : msg_without_block.get ());
			}
		}
	}
}

void nano::websocket::listener::broadcast (nano::websocket::message message_a)
{
	nano::lock_guard<nano::mutex> lk (sessions_mutex);
	for (auto & weak_session : sessions)
	{
		auto session_ptr (weak_session.lock ());
		if (session_ptr)
		{
			session_ptr->write (message_a);
		}
	}
}

void nano::websocket::listener::increase_subscriber_count (nano::websocket::topic const & topic_a)
{
	topic_subscriber_count[static_cast<std::size_t> (topic_a)] += 1;
}

void nano::websocket::listener::decrease_subscriber_count (nano::websocket::topic const & topic_a)
{
	auto & count (topic_subscriber_count[static_cast<std::size_t> (topic_a)]);
	release_assert (count > 0);
	count -= 1;
}

nano::websocket::message dto_to_message (rsnano::MessageDto & message_dto)
{
	auto ptree = static_cast<boost::property_tree::ptree *> (message_dto.contents);
	nano::websocket::message message_l (static_cast<nano::websocket::topic> (message_dto.topic));
	message_l.contents = *ptree;
	delete ptree;
	return message_l;
}

nano::websocket::message nano::websocket::message_builder::started_election (nano::block_hash const & hash_a)
{
	rsnano::MessageDto message_dto;
	rsnano::rsn_message_builder_started_election (hash_a.bytes.data (), &message_dto);
	return dto_to_message (message_dto);
}

nano::websocket::message nano::websocket::message_builder::stopped_election (nano::block_hash const & hash_a)
{
	rsnano::MessageDto message_dto;
	rsnano::rsn_message_builder_stopped_election (hash_a.bytes.data (), &message_dto);
	return dto_to_message (message_dto);
}

nano::websocket::message nano::websocket::message_builder::block_confirmed (
std::shared_ptr<nano::block> const & block_a,
nano::account const & account_a,
nano::amount const & amount_a,
std::string subtype,
bool include_block_a,
nano::election_status const & election_status_a,
std::vector<nano::vote_with_weight_info> const & election_votes_a,
nano::websocket::confirmation_options const & options_a)
{
	auto vec_handle = rsnano::rsn_vote_with_weight_info_vec_create ();
	for (const auto & info : election_votes_a)
	{
		auto dto{ info.into_dto () };
		rsnano::rsn_vote_with_weight_info_vec_push (vec_handle, &dto);
	}
	rsnano::MessageDto message_dto;
	rsnano::rsn_message_builder_block_confirmed (block_a->get_handle (),
	account_a.bytes.data (), amount_a.bytes.data (),
	subtype.c_str (), include_block_a, election_status_a.handle,
	vec_handle,
	options_a.handle,
	&message_dto);
	rsnano::rsn_vote_with_weight_info_vec_destroy (vec_handle);
	return dto_to_message (message_dto);
}

nano::websocket::message nano::websocket::message_builder::vote_received (std::shared_ptr<nano::vote> const & vote_a, nano::vote_code code_a)
{
	rsnano::MessageDto message_dto;
	rsnano::rsn_message_builder_vote_received (vote_a->get_handle (), static_cast<uint8_t> (code_a), &message_dto);
	return dto_to_message (message_dto);
}

nano::websocket::message nano::websocket::message_builder::work_generation (nano::work_version const version_a, nano::block_hash const & root_a, uint64_t work_a, uint64_t difficulty_a, uint64_t publish_threshold_a, std::chrono::milliseconds const & duration_a, std::string const & peer_a, std::vector<std::string> const & bad_peers_a, bool completed_a, bool cancelled_a)
{
	rsnano::string_vec bad_peers_vec (bad_peers_a);
	rsnano::MessageDto message_dto;
	rsnano::rsn_message_builder_work_generation (
	static_cast<uint8_t> (version_a),
	root_a.bytes.data (),
	work_a, difficulty_a, publish_threshold_a, duration_a.count (),
	peer_a.c_str (), bad_peers_vec.handle, completed_a, cancelled_a, &message_dto);
	return dto_to_message (message_dto);
}

nano::websocket::message nano::websocket::message_builder::work_cancelled (nano::work_version const version_a, nano::block_hash const & root_a, uint64_t const difficulty_a, uint64_t const publish_threshold_a, std::chrono::milliseconds const & duration_a, std::vector<std::string> const & bad_peers_a)
{
	return work_generation (version_a, root_a, 0, difficulty_a, publish_threshold_a, duration_a, "", bad_peers_a, false, true);
}

nano::websocket::message nano::websocket::message_builder::work_failed (nano::work_version const version_a, nano::block_hash const & root_a, uint64_t const difficulty_a, uint64_t const publish_threshold_a, std::chrono::milliseconds const & duration_a, std::vector<std::string> const & bad_peers_a)
{
	return work_generation (version_a, root_a, 0, difficulty_a, publish_threshold_a, duration_a, "", bad_peers_a, false, false);
}

nano::websocket::message nano::websocket::message_builder::bootstrap_started (std::string const & id_a, std::string const & mode_a)
{
	rsnano::MessageDto message_dto;
	rsnano::rsn_message_builder_bootstrap_started (id_a.c_str (), mode_a.c_str (), &message_dto);
	return dto_to_message (message_dto);
}

nano::websocket::message nano::websocket::message_builder::bootstrap_exited (std::string const & id_a, std::string const & mode_a, std::chrono::steady_clock::time_point const start_time_a, uint64_t const total_blocks_a)
{
	rsnano::MessageDto message_dto;
	auto duration = std::chrono::duration_cast<std::chrono::seconds> (std::chrono::steady_clock::now () - start_time_a).count ();
	rsnano::rsn_message_builder_bootstrap_exited (id_a.c_str (), mode_a.c_str (), duration, total_blocks_a, &message_dto);
	return dto_to_message (message_dto);
}

nano::websocket::message nano::websocket::message_builder::telemetry_received (nano::telemetry_data const & telemetry_data_a, nano::endpoint const & endpoint_a)
{
	nano::websocket::message message_l (nano::websocket::topic::telemetry);
	set_common_fields (message_l);

	// Telemetry information
	nano::jsonconfig telemetry_l;
	telemetry_data_a.serialize_json (telemetry_l, false);
	telemetry_l.put ("address", endpoint_a.address ());
	telemetry_l.put ("port", endpoint_a.port ());

	message_l.contents.add_child ("message", telemetry_l.get_tree ());
	return message_l;
}

nano::websocket::message nano::websocket::message_builder::new_block_arrived (nano::block const & block_a)
{
	nano::websocket::message message_l (nano::websocket::topic::new_unconfirmed_block);
	set_common_fields (message_l);

	boost::property_tree::ptree block_l;
	block_a.serialize_json (block_l);
	auto subtype (nano::state_subtype (block_a.sideband ().details ()));
	block_l.put ("subtype", subtype);

	message_l.contents.add_child ("message", block_l);
	return message_l;
}

void nano::websocket::message_builder::set_common_fields (nano::websocket::message & message_a)
{
	rsnano::MessageDto msg;
	msg.topic = static_cast<uint8_t> (message_a.topic);
	msg.contents = &message_a.contents;
	rsnano::rsn_websocket_set_common_fields (&msg);
}

rsnano::MessageDto nano::websocket::message::to_dto () const
{
	return { static_cast<uint8_t> (topic), (void *)&contents };
}

std::string nano::websocket::message::to_string () const
{
	std::ostringstream ostream;
	boost::property_tree::write_json (ostream, contents);
	ostream.flush ();
	return ostream.str ();
}

/*
 * websocket_server
 */

nano::websocket_server::websocket_server (nano::websocket::config & config_a, nano::node_observers & observers_a, nano::wallets & wallets_a, nano::ledger & ledger_a, boost::asio::io_context & io_ctx_a, nano::logger & logger_a) :
	config{ config_a },
	observers{ observers_a },
	wallets{ wallets_a },
	ledger{ ledger_a },
	io_ctx{ io_ctx_a },
	logger{ logger_a }
{
	if (!config.enabled)
	{
		return;
	}

	auto endpoint = nano::tcp_endpoint{ boost::asio::ip::make_address_v6 (config.address), config.port };
	server = std::make_shared<nano::websocket::listener> (logger, wallets, io_ctx, endpoint);

	observers.blocks.add ([this] (nano::election_status const & status_a, std::vector<nano::vote_with_weight_info> const & votes_a, nano::account const & account_a, nano::amount const & amount_a, bool is_state_send_a, bool is_state_epoch_a) {
		debug_assert (status_a.get_election_status_type () != nano::election_status_type::ongoing);

		if (server->any_subscriber (nano::websocket::topic::confirmation))
		{
			auto block_a = status_a.get_winner ();
			std::string subtype;
			if (is_state_send_a)
			{
				subtype = "send";
			}
			else if (block_a->type () == nano::block_type::state)
			{
				if (block_a->is_change ())
				{
					subtype = "change";
				}
				else if (is_state_epoch_a)
				{
					debug_assert (amount_a == 0 && ledger.is_epoch_link (block_a->link_field ().value ()));
					subtype = "epoch";
				}
				else
				{
					subtype = "receive";
				}
			}

			server->broadcast_confirmation (block_a, account_a, amount_a, subtype, status_a, votes_a);
		}
	});

	observers.active_started.add ([this] (nano::block_hash const & hash_a) {
		if (server->any_subscriber (nano::websocket::topic::started_election))
		{
			nano::websocket::message_builder builder;
			server->broadcast (builder.started_election (hash_a));
		}
	});

	observers.active_stopped.add ([this] (nano::block_hash const & hash_a) {
		if (server->any_subscriber (nano::websocket::topic::stopped_election))
		{
			nano::websocket::message_builder builder;
			server->broadcast (builder.stopped_election (hash_a));
		}
	});

	observers.telemetry.add ([this] (nano::telemetry_data const & telemetry_data, std::shared_ptr<nano::transport::channel> const & channel) {
		if (server->any_subscriber (nano::websocket::topic::telemetry))
		{
			nano::websocket::message_builder builder;
			server->broadcast (builder.telemetry_received (telemetry_data, channel->get_remote_endpoint ()));
		}
	});

	observers.vote.add ([this] (std::shared_ptr<nano::vote> vote_a, std::shared_ptr<nano::transport::channel> const & channel_a, nano::vote_code code_a) {
		if (server->any_subscriber (nano::websocket::topic::vote))
		{
			nano::websocket::message_builder builder;
			auto msg{ builder.vote_received (vote_a, code_a) };
			server->broadcast (msg);
		}
	});
}

void nano::websocket_server::start ()
{
	if (server)
	{
		server->run ();
	}
}

void nano::websocket_server::stop ()
{
	if (server)
	{
		server->stop ();
	}
}
