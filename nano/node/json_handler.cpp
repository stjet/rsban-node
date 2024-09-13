#include "nano/lib/numbers.hpp"
#include "nano/lib/rsnano.hpp"
#include "nano/lib/rsnanoutils.hpp"

#include <nano/lib/blocks.hpp>
#include <nano/lib/config.hpp>
#include <nano/lib/json_error_response.hpp>
#include <nano/lib/timer.hpp>
#include <nano/node/active_elections.hpp>
#include <nano/node/common.hpp>
#include <nano/node/election.hpp>
#include <nano/node/json_handler.hpp>
#include <nano/node/node.hpp>
#include <nano/node/node_rpc_config.hpp>
#include <nano/node/telemetry.hpp>
#include <nano/node/wallet.hpp>
#include <nano/secure/ledger.hpp>

#include <boost/property_tree/json_parser.hpp>
#include <boost/property_tree/ptree.hpp>

#include <algorithm>
#include <chrono>
#include <limits>
#include <vector>

namespace
{
void construct_json (nano::container_info_component * component, boost::property_tree::ptree & parent);
using ipc_json_handler_no_arg_func_map = std::unordered_map<std::string, std::function<void (nano::json_handler *)>>;
ipc_json_handler_no_arg_func_map create_ipc_json_handler_no_arg_func_map ();
auto ipc_json_handler_no_arg_funcs = create_ipc_json_handler_no_arg_func_map ();
bool block_confirmed (nano::node & node, nano::store::transaction & transaction, nano::block_hash const & hash, bool include_active, bool include_only_confirmed);
char const * epoch_as_string (nano::epoch);
}

nano::json_handler::json_handler (nano::node & node_a, nano::node_rpc_config const & node_rpc_config_a, std::string const & body_a, std::function<void (std::string const &)> const & response_a, std::function<void ()> stop_callback_a) :
	body (body_a),
	node (node_a),
	response (response_a),
	stop_callback (stop_callback_a),
	node_rpc_config (node_rpc_config_a)
{
}

std::function<void ()> nano::json_handler::create_worker_task (std::function<void (std::shared_ptr<nano::json_handler> const &)> const & action_a)
{
	return [rpc_l = shared_from_this (), action_a] () {
		try
		{
			action_a (rpc_l);
		}
		catch (std::runtime_error const &)
		{
			json_error_response (rpc_l->response, "Unable to parse JSON");
		}
		catch (...)
		{
			json_error_response (rpc_l->response, "Internal server error in RPC");
		}
	};
}

void nano::json_handler::process_request (bool unsafe_a)
{
	try
	{
		std::stringstream istream (body);
		boost::property_tree::read_json (istream, request);
		if (node_rpc_config.request_callback)
		{
			debug_assert (node.network_params.network.is_dev_network ());
			node_rpc_config.request_callback (request);
		}
		action = request.get<std::string> ("action");
		auto no_arg_func_iter = ipc_json_handler_no_arg_funcs.find (action);
		if (no_arg_func_iter != ipc_json_handler_no_arg_funcs.cend ())
		{
			// First try the map of options with no arguments
			no_arg_func_iter->second (this);
		}
		else
		{
			// Try the rest of the options
			if (action == "wallet_seed")
			{
				if (unsafe_a || node.network_params.network.is_dev_network ())
				{
					wallet_seed ();
				}
				else
				{
					json_error_response (response, "Unsafe RPC not allowed");
				}
			}
			else if (action == "chain")
			{
				chain ();
			}
			else if (action == "successors")
			{
				chain (true);
			}
			else if (action == "history")
			{
				response_l.put ("deprecated", "1");
				request.put ("head", request.get<std::string> ("hash"));
				account_history ();
			}
			else if (action == "knano_from_raw" || action == "krai_from_raw")
			{
				mnano_from_raw (nano::kxrb_ratio);
			}
			else if (action == "knano_to_raw" || action == "krai_to_raw")
			{
				mnano_to_raw (nano::kxrb_ratio);
			}
			else if (action == "rai_from_raw")
			{
				mnano_from_raw (nano::xrb_ratio);
			}
			else if (action == "rai_to_raw")
			{
				mnano_to_raw (nano::xrb_ratio);
			}
			else if (action == "mnano_from_raw" || action == "mrai_from_raw")
			{
				mnano_from_raw ();
			}
			else if (action == "mnano_to_raw" || action == "mrai_to_raw")
			{
				mnano_to_raw ();
			}
			else if (action == "nano_to_raw")
			{
				nano_to_raw ();
			}
			else if (action == "raw_to_nano")
			{
				raw_to_nano ();
			}
			else if (action == "password_valid")
			{
				password_valid ();
			}
			else if (action == "wallet_locked")
			{
				password_valid (true);
			}
			else
			{
				json_error_response (response, "Unknown command");
			}
		}
	}
	catch (std::runtime_error const &)
	{
		json_error_response (response, "Unable to parse JSON");
	}
	catch (...)
	{
		json_error_response (response, "Internal server error in RPC");
	}
}

void nano::json_handler::response_errors ()
{
	if (!ec && response_l.empty ())
	{
		// Return an error code if no response data was given
		ec = nano::error_rpc::empty_response;
	}
	if (ec)
	{
		boost::property_tree::ptree response_error;
		response_error.put ("error", ec.message ());
		std::stringstream ostream;
		boost::property_tree::write_json (ostream, response_error);
		response (ostream.str ());
	}
	else
	{
		std::stringstream ostream;
		boost::property_tree::write_json (ostream, response_l);
		response (ostream.str ());
	}
}

nano::wallet_id nano::json_handler::get_wallet_id ()
{
	if (!ec)
	{
		std::string wallet_text (request.get<std::string> ("wallet"));
		nano::wallet_id wallet;
		if (!wallet.decode_hex (wallet_text))
		{
			if (node.wallets.wallet_exists (wallet))
			{
				return wallet;
			}
			else
			{
				ec = nano::error_common::wallet_not_found;
			}
		}
		else
		{
			ec = nano::error_common::bad_wallet_number;
		}
	}
	return nano::wallet_id{};
}

nano::account nano::json_handler::account_impl (std::string account_text, std::error_code ec_a)
{
	nano::account result{};
	if (!ec)
	{
		if (account_text.empty ())
		{
			account_text = request.get<std::string> ("account");
		}
		if (result.decode_account (account_text))
		{
			ec = ec_a;
		}
		else if (account_text[3] == '-' || account_text[4] == '-')
		{
			// nano- and xrb- prefixes are deprecated
			response_l.put ("deprecated_account_format", "1");
		}
	}
	return result;
}

nano::account_info nano::json_handler::account_info_impl (store::transaction const & transaction_a, nano::account const & account_a)
{
	nano::account_info result;
	if (!ec)
	{
		auto info = node.ledger.any ().account_get (transaction_a, account_a);
		if (!info)
		{
			ec = nano::error_common::account_not_found;
			node.bootstrap_initiator.bootstrap_lazy (account_a, false, account_a.to_account ());
		}
		else
		{
			result = *info;
		}
	}
	return result;
}

nano::amount nano::json_handler::amount_impl ()
{
	nano::amount result (0);
	if (!ec)
	{
		std::string amount_text (request.get<std::string> ("amount"));
		if (result.decode_dec (amount_text))
		{
			ec = nano::error_common::invalid_amount;
		}
	}
	return result;
}

std::shared_ptr<nano::block> nano::json_handler::block_impl (bool signature_work_required)
{
	bool const json_block_l = request.get<bool> ("json_block", false);
	std::shared_ptr<nano::block> result{ nullptr };
	if (!ec)
	{
		boost::property_tree::ptree block_l;
		if (json_block_l)
		{
			block_l = request.get_child ("block");
		}
		else
		{
			std::string block_text (request.get<std::string> ("block"));
			std::stringstream block_stream (block_text);
			try
			{
				boost::property_tree::read_json (block_stream, block_l);
			}
			catch (...)
			{
				ec = nano::error_blocks::invalid_block;
			}
		}
		if (!ec)
		{
			if (!signature_work_required)
			{
				block_l.put ("signature", "0");
				block_l.put ("work", "0");
			}
			result = nano::deserialize_block_json (block_l);
			if (result == nullptr)
			{
				ec = nano::error_blocks::invalid_block;
			}
		}
	}
	return result;
}

nano::block_hash nano::json_handler::hash_impl (std::string search_text)
{
	nano::block_hash result (0);
	if (!ec)
	{
		std::string hash_text (request.get<std::string> (search_text));
		if (result.decode_hex (hash_text))
		{
			ec = nano::error_blocks::invalid_block_hash;
		}
	}
	return result;
}

nano::amount nano::json_handler::threshold_optional_impl ()
{
	nano::amount result (0);
	boost::optional<std::string> threshold_text (request.get_optional<std::string> ("threshold"));
	if (!ec && threshold_text.is_initialized ())
	{
		if (result.decode_dec (threshold_text.get ()))
		{
			ec = nano::error_common::bad_threshold;
		}
	}
	return result;
}

uint64_t nano::json_handler::work_optional_impl ()
{
	uint64_t result (0);
	boost::optional<std::string> work_text (request.get_optional<std::string> ("work"));
	if (!ec && work_text.is_initialized ())
	{
		if (nano::from_string_hex (work_text.get (), result))
		{
			ec = nano::error_common::bad_work_format;
		}
	}
	return result;
}

uint64_t nano::json_handler::difficulty_optional_impl (nano::work_version const version_a)
{
	auto difficulty (node.default_difficulty (version_a));
	boost::optional<std::string> difficulty_text (request.get_optional<std::string> ("difficulty"));
	if (!ec && difficulty_text.is_initialized ())
	{
		if (nano::from_string_hex (difficulty_text.get (), difficulty))
		{
			ec = nano::error_rpc::bad_difficulty_format;
		}
	}
	return difficulty;
}

uint64_t nano::json_handler::difficulty_ledger (nano::block const & block_a)
{
	nano::block_details details (nano::epoch::epoch_0, false, false, false);
	bool details_found (false);
	auto transaction (node.store.tx_begin_read ());
	// Previous block find
	std::shared_ptr<nano::block> block_previous (nullptr);
	auto previous (block_a.previous ());
	if (!previous.is_zero ())
	{
		block_previous = node.ledger.any ().block_get (*transaction, previous);
	}
	// Send check
	if (block_previous != nullptr)
	{
		auto is_send = node.ledger.any ().block_balance (*transaction, previous) > block_a.balance_field ().value ().number ();
		details = nano::block_details (nano::epoch::epoch_0, is_send, false, false);
		details_found = true;
	}
	// Epoch check
	if (block_previous != nullptr)
	{
		auto epoch = block_previous->sideband ().details ().epoch ();
		details = nano::block_details (epoch, details.is_send (), details.is_receive (), details.is_epoch ());
	}
	auto link = block_a.link_field ();
	if (link && !details.is_send ())
	{
		auto block_link (node.ledger.any ().block_get (*transaction, link.value ().as_block_hash ()));
		auto account = block_a.account_field ().value (); // Link is non-zero therefore it's a state block and has an account field;
		if (block_link != nullptr && node.ledger.any ().pending_get (*transaction, nano::pending_key{ account, link.value ().as_block_hash () }))
		{
			auto epoch = std::max (details.epoch (), block_link->sideband ().details ().epoch ());
			details = nano::block_details (epoch, details.is_send (), true, details.is_epoch ());
			details_found = true;
		}
	}
	return details_found ? node.network_params.work.threshold (block_a.work_version (), details) : node.default_difficulty (block_a.work_version ());
}

double nano::json_handler::multiplier_optional_impl (nano::work_version const version_a, uint64_t & difficulty)
{
	double multiplier (1.);
	boost::optional<std::string> multiplier_text (request.get_optional<std::string> ("multiplier"));
	if (!ec && multiplier_text.is_initialized ())
	{
		auto success = boost::conversion::try_lexical_convert<double> (multiplier_text.get (), multiplier);
		if (success && multiplier > 0.)
		{
			difficulty = nano::difficulty::from_multiplier (multiplier, node.default_difficulty (version_a));
		}
		else
		{
			ec = nano::error_rpc::bad_multiplier_format;
		}
	}
	return multiplier;
}

nano::work_version nano::json_handler::work_version_optional_impl (nano::work_version const default_a)
{
	nano::work_version result = default_a;
	boost::optional<std::string> version_text (request.get_optional<std::string> ("version"));
	if (!ec && version_text.is_initialized ())
	{
		if (*version_text == nano::to_string (nano::work_version::work_1))
		{
			result = nano::work_version::work_1;
		}
		else
		{
			ec = nano::error_rpc::bad_work_version;
		}
	}
	return result;
}

namespace
{
bool decode_unsigned (std::string const & text, uint64_t & number)
{
	bool result;
	std::size_t end;
	try
	{
		number = std::stoull (text, &end);
		result = false;
	}
	catch (std::invalid_argument const &)
	{
		result = true;
	}
	catch (std::out_of_range const &)
	{
		result = true;
	}
	result = result || end != text.size ();
	return result;
}
}

uint64_t nano::json_handler::count_impl ()
{
	uint64_t result (0);
	if (!ec)
	{
		std::string count_text (request.get<std::string> ("count"));
		if (decode_unsigned (count_text, result) || result == 0)
		{
			ec = nano::error_common::invalid_count;
		}
	}
	return result;
}

uint64_t nano::json_handler::count_optional_impl (uint64_t result)
{
	boost::optional<std::string> count_text (request.get_optional<std::string> ("count"));
	if (!ec && count_text.is_initialized ())
	{
		if (decode_unsigned (count_text.get (), result))
		{
			ec = nano::error_common::invalid_count;
		}
	}
	return result;
}

uint64_t nano::json_handler::offset_optional_impl (uint64_t result)
{
	boost::optional<std::string> offset_text (request.get_optional<std::string> ("offset"));
	if (!ec && offset_text.is_initialized ())
	{
		if (decode_unsigned (offset_text.get (), result))
		{
			ec = nano::error_rpc::invalid_offset;
		}
	}
	return result;
}

void nano::json_handler::account_balance ()
{
	auto account (account_impl ());
	if (!ec)
	{
		bool const include_only_confirmed = request.get<bool> ("include_only_confirmed", true);
		auto balance (node.balance_pending (account, include_only_confirmed));
		response_l.put ("balance", balance.first.convert_to<std::string> ());
		response_l.put ("pending", balance.second.convert_to<std::string> ());
		response_l.put ("receivable", balance.second.convert_to<std::string> ());
	}
	response_errors ();
}

void nano::json_handler::account_block_count ()
{
	auto account (account_impl ());
	if (!ec)
	{
		auto transaction (node.store.tx_begin_read ());
		auto info (account_info_impl (*transaction, account));
		if (!ec)
		{
			response_l.put ("block_count", std::to_string (info.block_count ()));
		}
	}
	response_errors ();
}

void nano::json_handler::account_create ()
{
	node.workers->push_task (create_worker_task ([] (std::shared_ptr<nano::json_handler> const & rpc_l) {
		auto wallet_id (rpc_l->get_wallet_id ());
		if (!rpc_l->ec)
		{
			bool const generate_work = rpc_l->request.get<bool> ("work", true);
			nano::account new_key;
			auto index_text (rpc_l->request.get_optional<std::string> ("index"));
			if (index_text.is_initialized ())
			{
				uint64_t index;
				if (decode_unsigned (index_text.get (), index) || index > static_cast<uint64_t> (std::numeric_limits<uint32_t>::max ()))
				{
					rpc_l->ec = nano::error_common::invalid_index;
				}
				else
				{
					auto error = rpc_l->node.wallets.deterministic_insert (wallet_id, static_cast<uint32_t> (index), generate_work, new_key);
					rpc_l->set_error (error);
				}
			}
			else
			{
				auto error = rpc_l->node.wallets.deterministic_insert (wallet_id, generate_work, new_key);
				rpc_l->set_error (error);
			}

			if (!rpc_l->ec)
			{
				rpc_l->response_l.put ("account", new_key.to_account ());
			}
		}
		rpc_l->response_errors ();
	}));
}

void nano::json_handler::account_get ()
{
	std::string key_text (request.get<std::string> ("key"));
	nano::public_key pub;
	if (!pub.decode_hex (key_text))
	{
		response_l.put ("account", pub.to_account ());
	}
	else
	{
		ec = nano::error_common::bad_public_key;
	}
	response_errors ();
}

void nano::json_handler::account_info ()
{
	auto account (account_impl ());
	if (!ec)
	{
		bool const representative = request.get<bool> ("representative", false);
		bool const weight = request.get<bool> ("weight", false);
		bool const pending = request.get<bool> ("pending", false);
		bool const receivable = request.get<bool> ("receivable", pending);
		bool const include_confirmed = request.get<bool> ("include_confirmed", false);
		auto transaction (node.store.tx_begin_read ());
		auto info (account_info_impl (*transaction, account));
		nano::confirmation_height_info confirmation_height_info;
		node.store.confirmation_height ().get (*transaction, account, confirmation_height_info);
		if (!ec)
		{
			response_l.put ("frontier", info.head ().to_string ());
			response_l.put ("open_block", info.open_block ().to_string ());
			response_l.put ("representative_block", node.ledger.representative (*transaction, info.head ()).to_string ());
			nano::amount balance_l (info.balance ());
			std::string balance;
			balance_l.encode_dec (balance);

			response_l.put ("balance", balance);

			nano::amount confirmed_balance_l;
			if (include_confirmed)
			{
				if (info.block_count () != confirmation_height_info.height ())
				{
					confirmed_balance_l = node.ledger.any ().block_balance (*transaction, confirmation_height_info.frontier ()).value_or (0);
				}
				else
				{
					// block_height and confirmed height are the same, so can just reuse balance
					confirmed_balance_l = balance_l;
				}
				std::string confirmed_balance;
				confirmed_balance_l.encode_dec (confirmed_balance);
				response_l.put ("confirmed_balance", confirmed_balance);
			}

			response_l.put ("modified_timestamp", std::to_string (info.modified ()));
			response_l.put ("block_count", std::to_string (info.block_count ()));
			response_l.put ("account_version", epoch_as_string (info.epoch ()));
			auto confirmed_frontier = confirmation_height_info.frontier ().to_string ();
			if (include_confirmed)
			{
				response_l.put ("confirmed_height", std::to_string (confirmation_height_info.height ()));
				response_l.put ("confirmed_frontier", confirmed_frontier);
			}
			else
			{
				// For backwards compatibility purposes
				response_l.put ("confirmation_height", std::to_string (confirmation_height_info.height ()));
				response_l.put ("confirmation_height_frontier", confirmed_frontier);
			}

			std::shared_ptr<nano::block> confirmed_frontier_block;
			if (include_confirmed && confirmation_height_info.height () > 0)
			{
				confirmed_frontier_block = node.ledger.any ().block_get (*transaction, confirmation_height_info.frontier ());
			}

			if (representative)
			{
				response_l.put ("representative", info.representative ().to_account ());
				if (include_confirmed)
				{
					nano::account confirmed_representative{};
					if (confirmed_frontier_block)
					{
						confirmed_representative = confirmed_frontier_block->representative_field ().value_or (0);
						if (confirmed_representative.is_zero ())
						{
							confirmed_representative = node.ledger.any ().block_get (*transaction, node.ledger.representative (*transaction, confirmation_height_info.frontier ()))->representative_field ().value ();
						}
					}

					response_l.put ("confirmed_representative", confirmed_representative.to_account ());
				}
			}
			if (weight)
			{
				auto account_weight (node.ledger.weight_exact (*transaction, account));
				response_l.put ("weight", account_weight.convert_to<std::string> ());
			}
			if (receivable)
			{
				auto account_receivable = node.ledger.account_receivable (*transaction, account);
				response_l.put ("pending", account_receivable.convert_to<std::string> ());
				response_l.put ("receivable", account_receivable.convert_to<std::string> ());

				if (include_confirmed)
				{
					auto account_receivable = node.ledger.account_receivable (*transaction, account, true);
					response_l.put ("confirmed_pending", account_receivable.convert_to<std::string> ());
					response_l.put ("confirmed_receivable", account_receivable.convert_to<std::string> ());
				}
			}
		}
	}
	response_errors ();
}

void nano::json_handler::account_key ()
{
	auto account (account_impl ());
	if (!ec)
	{
		response_l.put ("key", account.to_string ());
	}
	response_errors ();
}

void nano::json_handler::account_list ()
{
	auto wallet_id{ get_wallet_id () };
	if (!ec)
	{
		std::vector<nano::account> accounts;
		auto error = node.wallets.get_accounts (wallet_id, accounts);
		if (error == nano::wallets_error::none)
		{
			boost::property_tree::ptree accounts_json;
			for (const auto & account : accounts)
			{
				boost::property_tree::ptree entry;
				entry.put ("", account.to_account ());
				accounts_json.push_back (std::make_pair ("", entry));
			}
			response_l.add_child ("accounts", accounts_json);
		}
		else
		{
			set_error (error);
		}
	}
	response_errors ();
}

void nano::json_handler::account_move ()
{
	node.workers->push_task (create_worker_task ([] (std::shared_ptr<nano::json_handler> const & rpc_l) {
		auto wallet_id{ rpc_l->get_wallet_id () };
		if (!rpc_l->ec)
		{
			std::string source_text (rpc_l->request.get<std::string> ("source"));
			auto accounts_text (rpc_l->request.get_child ("accounts"));
			nano::wallet_id source;
			if (!source.decode_hex (source_text))
			{
				if (rpc_l->node.wallets.wallet_exists (source))
				{
					std::vector<nano::public_key> accounts;
					for (auto i (accounts_text.begin ()), n (accounts_text.end ()); i != n; ++i)
					{
						auto account (rpc_l->account_impl (i->second.get<std::string> ("")));
						accounts.push_back (account);
					}

					auto error{ rpc_l->node.wallets.move_accounts (source, wallet_id, accounts) };
					rpc_l->response_l.put ("moved", error ? "0" : "1");
				}
				else
				{
					rpc_l->ec = nano::error_rpc::source_not_found;
				}
			}
			else
			{
				rpc_l->ec = nano::error_rpc::bad_source;
			}
		}
		rpc_l->response_errors ();
	}));
}

void nano::json_handler::account_remove ()
{
	node.workers->push_task (create_worker_task ([] (std::shared_ptr<nano::json_handler> const & rpc_l) {
		auto wallet_id (rpc_l->get_wallet_id ());
		auto account (rpc_l->account_impl ());
		if (!rpc_l->ec)
		{
			auto error = rpc_l->node.wallets.remove_account (wallet_id, account);
			if (error == nano::wallets_error::none)
			{
				rpc_l->response_l.put ("removed", "1");
			}
			rpc_l->set_error (error);
		}

		rpc_l->response_errors ();
	}));
}

void nano::json_handler::account_representative ()
{
	auto account (account_impl ());
	if (!ec)
	{
		auto transaction (node.store.tx_begin_read ());
		auto info (account_info_impl (*transaction, account));
		if (!ec)
		{
			response_l.put ("representative", info.representative ().to_account ());
		}
	}
	response_errors ();
}

void nano::json_handler::account_representative_set ()
{
	node.workers->push_task (create_worker_task ([work_generation_enabled = node.work_generation_enabled ()] (std::shared_ptr<nano::json_handler> const & rpc_l) {
		auto wallet_id (rpc_l->get_wallet_id ());
		auto account (rpc_l->account_impl ());
		std::string representative_text (rpc_l->request.get<std::string> ("representative"));
		auto representative (rpc_l->account_impl (representative_text, nano::error_rpc::bad_representative_number));
		if (!rpc_l->ec)
		{
			auto work (rpc_l->work_optional_impl ());
			if (!rpc_l->ec && work)
			{
				auto block_transaction (rpc_l->node.store.tx_begin_read ());
				auto info (rpc_l->account_info_impl (*block_transaction, account));
				if (!rpc_l->ec)
				{
					nano::block_details details (info.epoch (), false, false, false);
					if (rpc_l->node.network_params.work.difficulty (nano::work_version::work_1, info.head (), work) < rpc_l->node.network_params.work.threshold (nano::work_version::work_1, details))
					{
						rpc_l->ec = nano::error_common::invalid_work;
					}
				}
			}
			else if (!rpc_l->ec) // work == 0
			{
				if (!work_generation_enabled)
				{
					rpc_l->ec = nano::error_common::disabled_work_generation;
				}
			}
			if (!rpc_l->ec)
			{
				bool generate_work (work == 0); // Disable work generation if "work" option is provided
				auto response_a (rpc_l->response);
				auto response_data (std::make_shared<boost::property_tree::ptree> (rpc_l->response_l));
				auto error = rpc_l->node.wallets.change_async (
				wallet_id, account, representative, [response_a, response_data] (std::shared_ptr<nano::block> const & block) {
					if (block != nullptr)
					{
						response_data->put ("block", block->hash ().to_string ());
						std::stringstream ostream;
						boost::property_tree::write_json (ostream, *response_data);
						response_a (ostream.str ());
					}
					else
					{
						json_error_response (response_a, "Error generating block");
					}
				},
				work, generate_work);

				rpc_l->set_error (error);
			}
		}
		// Because of change_async
		if (rpc_l->ec)
		{
			rpc_l->response_errors ();
		}
	}));
}

void nano::json_handler::account_weight ()
{
	auto account (account_impl ());
	if (!ec)
	{
		auto balance (node.weight (account));
		response_l.put ("weight", balance.convert_to<std::string> ());
	}
	response_errors ();
}

void nano::json_handler::accounts_balances ()
{
	boost::property_tree::ptree balances;
	boost::property_tree::ptree errors;
	auto transaction = node.store.tx_begin_read ();
	for (auto & account_from_request : request.get_child ("accounts"))
	{
		boost::property_tree::ptree entry;
		auto account = account_impl (account_from_request.second.data ());
		if (!ec)
		{
			bool const include_only_confirmed = request.get<bool> ("include_only_confirmed", true);
			auto balance = node.balance_pending (account, include_only_confirmed);
			entry.put ("balance", balance.first.convert_to<std::string> ());
			entry.put ("pending", balance.second.convert_to<std::string> ());
			entry.put ("receivable", balance.second.convert_to<std::string> ());
			balances.put_child (account_from_request.second.data (), entry);
			continue;
		}
		debug_assert (ec);
		errors.put (account_from_request.second.data (), ec.message ());
		ec = {};
	}
	if (!balances.empty ())
	{
		response_l.add_child ("balances", balances);
	}
	if (!errors.empty ())
	{
		response_l.add_child ("errors", errors);
	}
	response_errors ();
}

void nano::json_handler::accounts_representatives ()
{
	boost::property_tree::ptree representatives;
	boost::property_tree::ptree errors;
	auto transaction = node.store.tx_begin_read ();
	for (auto & account_from_request : request.get_child ("accounts"))
	{
		auto account = account_impl (account_from_request.second.data ());
		if (!ec)
		{
			auto info = account_info_impl (*transaction, account);
			if (!ec)
			{
				representatives.put (account_from_request.second.data (), info.representative ().to_account ());
				continue;
			}
		}
		debug_assert (ec);
		errors.put (account_from_request.second.data (), ec.message ());
		ec = {};
	}
	if (!representatives.empty ())
	{
		response_l.add_child ("representatives", representatives);
	}
	if (!errors.empty ())
	{
		response_l.add_child ("errors", errors);
	}
	response_errors ();
}

void nano::json_handler::accounts_create ()
{
	node.workers->push_task (create_worker_task ([] (std::shared_ptr<nano::json_handler> const & rpc_l) {
		auto wallet_id (rpc_l->get_wallet_id ());
		auto count (rpc_l->count_impl ());
		if (!rpc_l->ec)
		{
			bool const generate_work = rpc_l->request.get<bool> ("work", false);
			boost::property_tree::ptree accounts;
			for (auto i (0); accounts.size () < count; ++i)
			{
				nano::account new_key;
				auto error = rpc_l->node.wallets.deterministic_insert (wallet_id, generate_work, new_key);
				if (error != nano::wallets_error::none)
				{
					rpc_l->set_error (error);
					break;
				}
				boost::property_tree::ptree entry;
				entry.put ("", new_key.to_account ());
				accounts.push_back (std::make_pair ("", entry));
			}
			rpc_l->response_l.add_child ("accounts", accounts);
		}
		rpc_l->response_errors ();
	}));
}

void nano::json_handler::accounts_frontiers ()
{
	boost::property_tree::ptree frontiers;
	boost::property_tree::ptree errors;
	auto transaction = node.store.tx_begin_read ();
	for (auto & account_from_request : request.get_child ("accounts"))
	{
		auto account = account_impl (account_from_request.second.data ());
		if (!ec)
		{
			auto latest = node.ledger.any ().account_head (*transaction, account);
			if (!latest.is_zero ())
			{
				frontiers.put (account.to_account (), latest.to_string ());
				continue;
			}
			else
			{
				ec = nano::error_common::account_not_found;
			}
		}
		debug_assert (ec);
		errors.put (account_from_request.second.data (), ec.message ());
		ec = {};
	}
	if (!frontiers.empty ())
	{
		response_l.add_child ("frontiers", frontiers);
	}
	if (!errors.empty ())
	{
		response_l.add_child ("errors", errors);
	}
	response_errors ();
}

void nano::json_handler::accounts_pending ()
{
	response_l.put ("deprecated", "1");
	accounts_receivable ();
}

void nano::json_handler::accounts_receivable ()
{
	auto count (count_optional_impl ());
	auto threshold (threshold_optional_impl ());
	bool const source = request.get<bool> ("source", false);
	bool const include_active = request.get<bool> ("include_active", false);
	bool const include_only_confirmed = request.get<bool> ("include_only_confirmed", true);
	bool const sorting = request.get<bool> ("sorting", false);
	auto simple (threshold.is_zero () && !source && !sorting); // if simple, response is a list of hashes for each account
	boost::property_tree::ptree pending;
	auto transaction = node.store.tx_begin_read ();
	for (auto & accounts : request.get_child ("accounts"))
	{
		auto account (account_impl (accounts.second.data ()));
		if (!ec)
		{
			boost::property_tree::ptree peers_l;
			for (auto current = node.ledger.any ().receivable_upper_bound (*transaction, account, 0); !current.is_end () && peers_l.size () < count; ++current)
			{
				auto const & [key, info] = *current;
				if (include_only_confirmed && !node.ledger.confirmed ().block_exists_or_pruned (*transaction, key.hash))
				{
					continue;
				}
				if (simple)
				{
					boost::property_tree::ptree entry;
					entry.put ("", key.hash.to_string ());
					peers_l.push_back (std::make_pair ("", entry));
					continue;
				}
				if (info.amount.number () < threshold.number ())
				{
					continue;
				}

				if (source)
				{
					boost::property_tree::ptree pending_tree;
					pending_tree.put ("amount", info.amount.number ().template convert_to<std::string> ());
					pending_tree.put ("source", info.source.to_account ());
					peers_l.add_child (key.hash.to_string (), pending_tree);
				}
				else
				{
					peers_l.put (key.hash.to_string (), info.amount.number ().template convert_to<std::string> ());
				}
			}
			if (sorting && !simple)
			{
				if (source)
				{
					peers_l.sort ([] (auto const & child1, auto const & child2) -> bool {
						return child1.second.template get<nano::uint128_t> ("amount") > child2.second.template get<nano::uint128_t> ("amount");
					});
				}
				else
				{
					peers_l.sort ([] (auto const & child1, auto const & child2) -> bool {
						return child1.second.template get<nano::uint128_t> ("") > child2.second.template get<nano::uint128_t> ("");
					});
				}
			}
			if (!peers_l.empty ())
			{
				pending.add_child (account.to_account (), peers_l);
			}
		}
	}
	response_l.add_child ("blocks", pending);
	response_errors ();
}

void nano::json_handler::active_difficulty ()
{
	auto include_trend (request.get<bool> ("include_trend", false));
	auto const multiplier_active = 1.0;
	auto const default_difficulty (node.default_difficulty (nano::work_version::work_1));
	auto const default_receive_difficulty (node.default_receive_difficulty (nano::work_version::work_1));
	auto const receive_current_denormalized (node.network_params.work.denormalized_multiplier (multiplier_active, node.network_params.work.get_epoch_2_receive ()));
	response_l.put ("deprecated", "1");
	response_l.put ("network_minimum", nano::to_string_hex (default_difficulty));
	response_l.put ("network_receive_minimum", nano::to_string_hex (default_receive_difficulty));
	response_l.put ("network_current", nano::to_string_hex (nano::difficulty::from_multiplier (multiplier_active, default_difficulty)));
	response_l.put ("network_receive_current", nano::to_string_hex (nano::difficulty::from_multiplier (receive_current_denormalized, default_receive_difficulty)));
	response_l.put ("multiplier", 1.0);
	if (include_trend)
	{
		boost::property_tree::ptree difficulty_trend_l;

		// To keep this RPC backwards-compatible
		boost::property_tree::ptree entry;
		entry.put ("", "1.000000000000000");
		difficulty_trend_l.push_back (std::make_pair ("", entry));

		response_l.add_child ("difficulty_trend", difficulty_trend_l);
	}
	response_errors ();
}

void nano::json_handler::available_supply ()
{
	auto genesis_balance (node.balance (node.network_params.ledger.genesis->account_field ().value ())); // Cold storage genesis
	auto landing_balance (node.balance (nano::account ("059F68AAB29DE0D3A27443625C7EA9CDDB6517A8B76FE37727EF6A4D76832AD5"))); // Active unavailable account
	auto faucet_balance (node.balance (nano::account ("8E319CE6F3025E5B2DF66DA7AB1467FE48F1679C13DD43BFDB29FA2E9FC40D3B"))); // Faucet account
	auto burned_balance ((node.balance_pending (nano::account{}, false)).second); // Burning 0 account
	auto available (nano::dev::constants.genesis_amount - genesis_balance - landing_balance - faucet_balance - burned_balance);
	response_l.put ("available", available.convert_to<std::string> ());
	response_errors ();
}

void nano::json_handler::block_info ()
{
	auto hash (hash_impl ());
	if (!ec)
	{
		auto transaction (node.store.tx_begin_read ());
		auto block (node.ledger.any ().block_get (*transaction, hash));
		if (block != nullptr)
		{
			auto account = block->account ();
			response_l.put ("block_account", account.to_account ());
			auto amount = node.ledger.any ().block_amount (*transaction, hash);
			if (amount)
			{
				response_l.put ("amount", amount.value ().number ().convert_to<std::string> ());
			}
			auto balance = node.ledger.any ().block_balance (*transaction, hash);
			response_l.put ("balance", balance.value ().number ().convert_to<std::string> ());
			response_l.put ("height", std::to_string (block->sideband ().height ()));
			response_l.put ("local_timestamp", std::to_string (block->sideband ().timestamp ()));
			response_l.put ("successor", block->sideband ().successor ().to_string ());
			auto confirmed (node.ledger.confirmed ().block_exists_or_pruned (*transaction, hash));
			response_l.put ("confirmed", confirmed);

			bool json_block_l = request.get<bool> ("json_block", false);
			if (json_block_l)
			{
				boost::property_tree::ptree block_node_l;
				block->serialize_json (block_node_l);
				response_l.add_child ("contents", block_node_l);
			}
			else
			{
				std::string contents;
				block->serialize_json (contents);
				response_l.put ("contents", contents);
			}
			if (block->type () == nano::block_type::state)
			{
				auto subtype (nano::state_subtype (block->sideband ().details ()));
				response_l.put ("subtype", subtype);
			}
		}
		else
		{
			ec = nano::error_blocks::not_found;
		}
	}
	response_errors ();
}

void nano::json_handler::block_confirm ()
{
	auto hash (hash_impl ());
	if (!ec)
	{
		auto transaction (node.store.tx_begin_read ());
		auto block_l (node.ledger.any ().block_get (*transaction, hash));
		if (block_l != nullptr)
		{
			if (!node.ledger.confirmed ().block_exists_or_pruned (*transaction, hash))
			{
				// Start new confirmation for unconfirmed (or not being confirmed) block
				if (!node.confirming_set.exists (hash))
				{
					node.start_election (std::move (block_l));
				}
			}
			else
			{
				// Add record in confirmation history for confirmed block
				nano::election_status status{};
				status.set_winner (block_l);
				status.set_election_end (std::chrono::duration_cast<std::chrono::milliseconds> (std::chrono::system_clock::now ().time_since_epoch ()));
				status.set_block_count (1);
				status.set_election_status_type (nano::election_status_type::active_confirmation_height);
				node.active.insert_recently_cemented (status);
			}
			response_l.put ("started", "1");
		}
		else
		{
			ec = nano::error_blocks::not_found;
		}
	}
	response_errors ();
}

void nano::json_handler::blocks ()
{
	bool const json_block_l = request.get<bool> ("json_block", false);
	boost::property_tree::ptree blocks;
	auto transaction (node.store.tx_begin_read ());
	for (boost::property_tree::ptree::value_type & hashes : request.get_child ("hashes"))
	{
		if (!ec)
		{
			std::string hash_text = hashes.second.data ();
			nano::block_hash hash;
			if (!hash.decode_hex (hash_text))
			{
				auto block (node.ledger.any ().block_get (*transaction, hash));
				if (block != nullptr)
				{
					if (json_block_l)
					{
						boost::property_tree::ptree block_node_l;
						block->serialize_json (block_node_l);
						blocks.add_child (hash_text, block_node_l);
					}
					else
					{
						std::string contents;
						block->serialize_json (contents);
						blocks.put (hash_text, contents);
					}
				}
				else
				{
					ec = nano::error_blocks::not_found;
				}
			}
			else
			{
				ec = nano::error_blocks::bad_hash_number;
			}
		}
	}
	response_l.add_child ("blocks", blocks);
	response_errors ();
}

void nano::json_handler::blocks_info ()
{
	bool const pending = request.get<bool> ("pending", false);
	bool const receivable = request.get<bool> ("receivable", pending);
	bool const receive_hash = request.get<bool> ("receive_hash", false);
	bool const source = request.get<bool> ("source", false);
	bool const json_block_l = request.get<bool> ("json_block", false);
	bool const include_not_found = request.get<bool> ("include_not_found", false);

	boost::property_tree::ptree blocks;
	boost::property_tree::ptree blocks_not_found;
	auto transaction (node.store.tx_begin_read ());
	for (boost::property_tree::ptree::value_type & hashes : request.get_child ("hashes"))
	{
		if (!ec)
		{
			std::string hash_text = hashes.second.data ();
			nano::block_hash hash;
			if (!hash.decode_hex (hash_text))
			{
				auto block (node.ledger.any ().block_get (*transaction, hash));
				if (block != nullptr)
				{
					boost::property_tree::ptree entry;
					auto account = block->account ();
					entry.put ("block_account", account.to_account ());
					auto amount (node.ledger.any ().block_amount (*transaction, hash));
					if (amount)
					{
						entry.put ("amount", amount.value ().number ().convert_to<std::string> ());
					}
					auto balance = block->balance ();
					entry.put ("balance", balance.number ().convert_to<std::string> ());
					entry.put ("height", std::to_string (block->sideband ().height ()));
					entry.put ("local_timestamp", std::to_string (block->sideband ().timestamp ()));
					entry.put ("successor", block->sideband ().successor ().to_string ());
					auto confirmed (node.ledger.confirmed ().block_exists_or_pruned (*transaction, hash));
					entry.put ("confirmed", confirmed);

					if (json_block_l)
					{
						boost::property_tree::ptree block_node_l;
						block->serialize_json (block_node_l);
						entry.add_child ("contents", block_node_l);
					}
					else
					{
						std::string contents;
						block->serialize_json (contents);
						entry.put ("contents", contents);
					}
					if (block->type () == nano::block_type::state)
					{
						auto subtype (nano::state_subtype (block->sideband ().details ()));
						entry.put ("subtype", subtype);
					}
					if (receivable || receive_hash)
					{
						if (!block->is_send ())
						{
							if (receivable)
							{
								entry.put ("pending", "0");
								entry.put ("receivable", "0");
							}
							if (receive_hash)
							{
								entry.put ("receive_hash", nano::block_hash (0).to_string ());
							}
						}
						else if (node.ledger.any ().pending_get (*transaction, nano::pending_key{ block->destination (), hash }))
						{
							if (receivable)
							{
								entry.put ("pending", "1");
								entry.put ("receivable", "1");
							}
							if (receive_hash)
							{
								entry.put ("receive_hash", nano::block_hash (0).to_string ());
							}
						}
						else
						{
							if (receivable)
							{
								entry.put ("pending", "0");
								entry.put ("receivable", "0");
							}
							if (receive_hash)
							{
								std::shared_ptr<nano::block> receive_block = node.ledger.find_receive_block_by_send_hash (*transaction, block->destination (), hash);
								std::string receive_hash = receive_block ? receive_block->hash ().to_string () : nano::block_hash (0).to_string ();
								entry.put ("receive_hash", receive_hash);
							}
						}
					}
					if (source)
					{
						if (!block->is_receive () || !node.ledger.any ().block_exists (*transaction, block->source ()))
						{
							entry.put ("source_account", "0");
						}
						else
						{
							auto block_a = node.ledger.any ().block_get (*transaction, block->source ());
							release_assert (block_a);
							entry.put ("source_account", block_a->account ().to_account ());
						}
					}
					blocks.push_back (std::make_pair (hash_text, entry));
				}
				else if (include_not_found)
				{
					boost::property_tree::ptree entry;
					entry.put ("", hash_text);
					blocks_not_found.push_back (std::make_pair ("", entry));
				}
				else
				{
					ec = nano::error_blocks::not_found;
				}
			}
			else
			{
				ec = nano::error_blocks::bad_hash_number;
			}
		}
	}
	if (!ec)
	{
		response_l.add_child ("blocks", blocks);
		if (include_not_found)
		{
			response_l.add_child ("blocks_not_found", blocks_not_found);
		}
	}
	response_errors ();
}

void nano::json_handler::block_account ()
{
	auto hash (hash_impl ());
	if (!ec)
	{
		auto transaction (node.store.tx_begin_read ());
		auto block = node.ledger.any ().block_get (*transaction, hash);
		if (block)
		{
			response_l.put ("account", block->account ().to_account ());
		}
		else
		{
			ec = nano::error_blocks::not_found;
		}
	}
	response_errors ();
}

void nano::json_handler::block_count ()
{
	response_l.put ("count", std::to_string (node.ledger.block_count ()));
	response_l.put ("unchecked", std::to_string (node.unchecked.count ()));
	response_l.put ("cemented", std::to_string (node.ledger.cemented_count ()));
	if (node.flags.enable_pruning ())
	{
		response_l.put ("full", std::to_string (node.ledger.block_count () - node.ledger.pruned_count ()));
		response_l.put ("pruned", std::to_string (node.ledger.pruned_count ()));
	}
	response_errors ();
}

void nano::json_handler::set_error (nano::wallets_error const & error)
{
	switch (error)
	{
		case nano::wallets_error::none:
			break;
		case nano::wallets_error::wallet_not_found:
			ec = nano::error_common::wallet_not_found;
			break;
		case nano::wallets_error::wallet_locked:
			ec = nano::error_common::wallet_locked;
			break;
		case nano::wallets_error::account_not_found:
			ec = nano::error_common::account_not_found_wallet;
			break;
		case nano::wallets_error::bad_public_key:
			ec = nano::error_common::bad_public_key;
			break;
		default:
			ec = nano::error_common::generic;
			break;
	}
}

void nano::json_handler::block_create ()
{
	std::string type (request.get<std::string> ("type"));
	nano::wallet_id wallet_id (0);
	// Default to work_1 if not specified
	auto work_version (work_version_optional_impl (nano::work_version::work_1));
	auto difficulty_l (difficulty_optional_impl (work_version));
	boost::optional<std::string> wallet_text (request.get_optional<std::string> ("wallet"));
	if (!ec && wallet_text.is_initialized ())
	{
		if (wallet_id.decode_hex (wallet_text.get ()))
		{
			ec = nano::error_common::bad_wallet_number;
		}
	}
	nano::account account{};
	boost::optional<std::string> account_text (request.get_optional<std::string> ("account"));
	if (!ec && account_text.is_initialized ())
	{
		account = account_impl (account_text.get ());
	}
	nano::account representative{};
	boost::optional<std::string> representative_text (request.get_optional<std::string> ("representative"));
	if (!ec && representative_text.is_initialized ())
	{
		representative = account_impl (representative_text.get (), nano::error_rpc::bad_representative_number);
	}
	nano::account destination{};
	boost::optional<std::string> destination_text (request.get_optional<std::string> ("destination"));
	if (!ec && destination_text.is_initialized ())
	{
		destination = account_impl (destination_text.get (), nano::error_rpc::bad_destination);
	}
	nano::block_hash source (0);
	boost::optional<std::string> source_text (request.get_optional<std::string> ("source"));
	if (!ec && source_text.is_initialized ())
	{
		if (source.decode_hex (source_text.get ()))
		{
			ec = nano::error_rpc::bad_source;
		}
	}
	nano::amount amount (0);
	boost::optional<std::string> amount_text (request.get_optional<std::string> ("amount"));
	if (!ec && amount_text.is_initialized ())
	{
		if (amount.decode_dec (amount_text.get ()))
		{
			ec = nano::error_common::invalid_amount;
		}
	}
	auto work (work_optional_impl ());
	nano::raw_key prv;
	prv.clear ();
	nano::block_hash previous (0);
	nano::amount balance (0);
	if (work == 0 && !node.work_generation_enabled ())
	{
		ec = nano::error_common::disabled_work_generation;
	}
	if (!ec && wallet_id != 0 && account != 0)
	{
		auto error = node.wallets.fetch (wallet_id, account, prv);
		if (error == nano::wallets_error::none)
		{
			auto block_transaction (node.store.tx_begin_read ());
			previous = node.ledger.any ().account_head (*block_transaction, account);
			balance = node.ledger.any ().account_balance (*block_transaction, account).value_or (0);
		}
		set_error (error);
	}
	boost::optional<std::string> key_text (request.get_optional<std::string> ("key"));
	if (!ec && key_text.is_initialized ())
	{
		if (prv.decode_hex (key_text.get ()))
		{
			ec = nano::error_common::bad_private_key;
		}
	}
	boost::optional<std::string> previous_text (request.get_optional<std::string> ("previous"));
	if (!ec && previous_text.is_initialized ())
	{
		if (previous.decode_hex (previous_text.get ()))
		{
			ec = nano::error_rpc::bad_previous;
		}
	}
	boost::optional<std::string> balance_text (request.get_optional<std::string> ("balance"));
	if (!ec && balance_text.is_initialized ())
	{
		if (balance.decode_dec (balance_text.get ()))
		{
			ec = nano::error_rpc::invalid_balance;
		}
	}
	nano::link link (0);
	boost::optional<std::string> link_text (request.get_optional<std::string> ("link"));
	if (!ec && link_text.is_initialized ())
	{
		if (link.decode_account (link_text.get ()))
		{
			if (link.decode_hex (link_text.get ()))
			{
				ec = nano::error_rpc::bad_link;
			}
		}
	}
	else
	{
		// Retrieve link from source or destination
		if (source.is_zero ())
		{
			link = destination;
		}
		else
		{
			link = source;
		}
	}
	if (!ec)
	{
		auto rpc_l (shared_from_this ());
		// Serializes the block contents to the RPC response
		auto block_response_put_l = [rpc_l, this] (nano::block const & block_a) {
			boost::property_tree::ptree response_l;
			response_l.put ("hash", block_a.hash ().to_string ());
			response_l.put ("difficulty", nano::to_string_hex (rpc_l->node.network_params.work.difficulty (block_a)));
			bool json_block_l = request.get<bool> ("json_block", false);
			if (json_block_l)
			{
				boost::property_tree::ptree block_node_l;
				block_a.serialize_json (block_node_l);
				response_l.add_child ("block", block_node_l);
			}
			else
			{
				std::string contents;
				block_a.serialize_json (contents);
				response_l.put ("block", contents);
			}
			std::stringstream ostream;
			boost::property_tree::write_json (ostream, response_l);
			rpc_l->response (ostream.str ());
		};
		// Wrapper from argument to lambda capture, to extend the block's scope
		auto get_callback_l = [rpc_l, block_response_put_l] (std::shared_ptr<nano::block> const & block_a) {
			// Callback upon work generation success or failure
			return [block_a, rpc_l, block_response_put_l] (std::optional<uint64_t> const & work_a) {
				if (block_a != nullptr)
				{
					if (work_a.has_value ())
					{
						block_a->block_work_set (*work_a);
						block_response_put_l (*block_a);
					}
					else
					{
						rpc_l->ec = nano::error_common::failure_work_generation;
					}
				}
				else
				{
					rpc_l->ec = nano::error_common::generic;
				}
				if (rpc_l->ec)
				{
					rpc_l->response_errors ();
				}
			};
		};
		if (prv != 0)
		{
			nano::account pub (nano::pub_key (prv));
			// Fetching account balance & previous for send blocks (if aren't given directly)
			if (!previous_text.is_initialized () && !balance_text.is_initialized ())
			{
				auto transaction (node.store.tx_begin_read ());
				previous = node.ledger.any ().account_head (*transaction, pub);
				balance = node.ledger.any ().account_balance (*transaction, pub).value_or (0);
			}
			// Double check current balance if previous block is specified
			else if (previous_text.is_initialized () && balance_text.is_initialized () && type == "send")
			{
				auto transaction (node.store.tx_begin_read ());
				if (node.ledger.any ().block_exists (*transaction, previous) && node.ledger.any ().block_balance (*transaction, previous) != balance.number ())
				{
					ec = nano::error_rpc::block_create_balance_mismatch;
				}
			}
			// Check for incorrect account key
			if (!ec && account_text.is_initialized ())
			{
				if (account != pub)
				{
					ec = nano::error_rpc::block_create_public_key_mismatch;
				}
			}
			nano::block_builder builder_l;
			std::shared_ptr<nano::block> block_l{ nullptr };
			nano::root root_l;
			std::error_code ec_build;
			if (type == "state")
			{
				if (previous_text.is_initialized () && !representative.is_zero () && (!link.is_zero () || link_text.is_initialized ()))
				{
					block_l = builder_l.state ()
							  .account (pub)
							  .previous (previous)
							  .representative (representative)
							  .balance (balance)
							  .link (link)
							  .sign (prv, pub)
							  .build (ec_build);
					if (previous.is_zero ())
					{
						root_l = pub;
					}
					else
					{
						root_l = previous;
					}
				}
				else
				{
					ec = nano::error_rpc::block_create_requirements_state;
				}
			}
			else if (type == "open")
			{
				if (representative != 0 && source != 0)
				{
					block_l = builder_l.open ()
							  .account (pub)
							  .source (source)
							  .representative (representative)
							  .sign (prv, pub)
							  .build (ec_build);
					root_l = pub;
				}
				else
				{
					ec = nano::error_rpc::block_create_requirements_open;
				}
			}
			else if (type == "receive")
			{
				if (source != 0 && previous != 0)
				{
					block_l = builder_l.receive ()
							  .previous (previous)
							  .source (source)
							  .sign (prv, pub)
							  .build (ec_build);
					root_l = previous;
				}
				else
				{
					ec = nano::error_rpc::block_create_requirements_receive;
				}
			}
			else if (type == "change")
			{
				if (representative != 0 && previous != 0)
				{
					block_l = builder_l.change ()
							  .previous (previous)
							  .representative (representative)
							  .sign (prv, pub)
							  .build (ec_build);
					root_l = previous;
				}
				else
				{
					ec = nano::error_rpc::block_create_requirements_change;
				}
			}
			else if (type == "send")
			{
				if (destination != 0 && previous != 0 && balance != 0 && amount != 0)
				{
					if (balance.number () >= amount.number ())
					{
						block_l = builder_l.send ()
								  .previous (previous)
								  .destination (destination)
								  .balance (balance.number () - amount.number ())
								  .sign (prv, pub)
								  .build (ec_build);
						root_l = previous;
					}
					else
					{
						ec = nano::error_common::insufficient_balance;
					}
				}
				else
				{
					ec = nano::error_rpc::block_create_requirements_send;
				}
			}
			else
			{
				ec = nano::error_blocks::invalid_type;
			}
			if (!ec && (!ec_build || ec_build == nano::error_common::missing_work))
			{
				if (work == 0)
				{
					// Difficulty calculation
					if (request.count ("difficulty") == 0)
					{
						difficulty_l = difficulty_ledger (*block_l);
					}
					node.work_generate (work_version, root_l, difficulty_l, get_callback_l (block_l), nano::account (pub));
				}
				else
				{
					block_l->block_work_set (work);
					block_response_put_l (*block_l);
				}
			}
		}
		else
		{
			ec = nano::error_rpc::block_create_key_required;
		}
	}
	// Because of callback
	if (ec)
	{
		response_errors ();
	}
}

void nano::json_handler::block_hash ()
{
	auto block (block_impl (true));

	if (!ec)
	{
		response_l.put ("hash", block->hash ().to_string ());
	}
	response_errors ();
}

void nano::json_handler::bootstrap ()
{
	std::string address_text = request.get<std::string> ("address");
	std::string port_text = request.get<std::string> ("port");
	boost::system::error_code address_ec;
	auto address (boost::asio::ip::make_address_v6 (address_text, address_ec));
	if (!address_ec)
	{
		uint16_t port;
		if (!nano::parse_port (port_text, port))
		{
			if (!node.flags.disable_legacy_bootstrap ())
			{
				std::string bootstrap_id (request.get<std::string> ("id", ""));
				node.connect (nano::endpoint (address, port));
				node.bootstrap_initiator.bootstrap (nano::endpoint (address, port), bootstrap_id);
				response_l.put ("success", "");
			}
			else
			{
				ec = nano::error_rpc::disabled_bootstrap_legacy;
			}
		}
		else
		{
			ec = nano::error_common::invalid_port;
		}
	}
	else
	{
		ec = nano::error_common::invalid_ip_address;
	}
	response_errors ();
}

void nano::json_handler::bootstrap_any ()
{
	bool const force = request.get<bool> ("force", false);
	if (!node.flags.disable_legacy_bootstrap ())
	{
		nano::account start_account{};
		boost::optional<std::string> account_text (request.get_optional<std::string> ("account"));
		if (account_text.is_initialized ())
		{
			start_account = account_impl (account_text.get ());
		}
		std::string bootstrap_id (request.get<std::string> ("id", ""));
		node.bootstrap_initiator.bootstrap (force, bootstrap_id, std::numeric_limits<uint32_t>::max (), start_account);
		response_l.put ("success", "");
	}
	else
	{
		ec = nano::error_rpc::disabled_bootstrap_legacy;
	}
	response_errors ();
}

void nano::json_handler::bootstrap_lazy ()
{
	auto hash (hash_impl ());
	bool const force = request.get<bool> ("force", false);
	if (!ec)
	{
		if (!node.flags.disable_lazy_bootstrap ())
		{
			auto existed = node.bootstrap_initiator.has_lazy_attempt ();
			std::string bootstrap_id (request.get<std::string> ("id", ""));
			auto key_inserted (node.bootstrap_initiator.bootstrap_lazy (hash, force, bootstrap_id));
			bool started = !existed && key_inserted;
			response_l.put ("started", started ? "1" : "0");
			response_l.put ("key_inserted", key_inserted ? "1" : "0");
		}
		else
		{
			ec = nano::error_rpc::disabled_bootstrap_lazy;
		}
	}
	response_errors ();
}

/*
 * @warning This is an internal/diagnostic RPC, do not rely on its interface being stable
 */
void nano::json_handler::bootstrap_status ()
{
	auto attempts_count (node.bootstrap_initiator.attempts.size ());
	response_l.put ("bootstrap_threads", std::to_string (node.config->bootstrap_initiator_threads));
	response_l.put ("running_attempts_count", std::to_string (attempts_count));
	response_l.put ("total_attempts_count", std::to_string (node.bootstrap_initiator.attempts.total_attempts ()));
	boost::property_tree::ptree connections;
	node.bootstrap_initiator.connections->bootstrap_status (connections, attempts_count);
	response_l.add_child ("connections", connections);
	response_l.add_child ("attempts", node.bootstrap_initiator.attempts.attempts_information ());
	response_errors ();
}

void nano::json_handler::chain (bool successors)
{
	successors = successors != request.get<bool> ("reverse", false);
	auto hash (hash_impl ("block"));
	auto count (count_impl ());
	auto offset (offset_optional_impl (0));
	if (!ec)
	{
		boost::property_tree::ptree blocks;
		auto transaction (node.store.tx_begin_read ());
		while (!hash.is_zero () && blocks.size () < count)
		{
			auto block_l (node.ledger.any ().block_get (*transaction, hash));
			if (block_l != nullptr)
			{
				if (offset > 0)
				{
					--offset;
				}
				else
				{
					boost::property_tree::ptree entry;
					entry.put ("", hash.to_string ());
					blocks.push_back (std::make_pair ("", entry));
				}
				hash = successors ? node.ledger.any ().block_successor (*transaction, hash).value_or (0) : block_l->previous ();
			}
			else
			{
				hash.clear ();
			}
		}
		response_l.add_child ("blocks", blocks);
	}
	response_errors ();
}

void nano::json_handler::confirmation_active ()
{
	uint64_t announcements (0);
	uint64_t confirmed (0);
	boost::optional<std::string> announcements_text (request.get_optional<std::string> ("announcements"));
	if (announcements_text.is_initialized ())
	{
		announcements = strtoul (announcements_text.get ().c_str (), NULL, 10);
	}
	boost::property_tree::ptree elections;
	auto active_elections = node.active.list_active ();
	for (auto const & election : active_elections)
	{
		if (election->get_confirmation_request_count () >= announcements)
		{
			if (!node.active.confirmed (*election))
			{
				boost::property_tree::ptree entry;
				entry.put ("", election->qualified_root ().to_string ());
				elections.push_back (std::make_pair ("", entry));
			}
			else
			{
				++confirmed;
			}
		}
	}
	response_l.add_child ("confirmations", elections);
	response_l.put ("unconfirmed", elections.size ());
	response_l.put ("confirmed", confirmed);
	response_errors ();
}

void nano::json_handler::election_statistics ()
{
	auto active_elections = node.active.list_active ();
	unsigned manual_count = 0;
	unsigned priority_count = 0;
	unsigned hinted_count = 0;
	unsigned optimistic_count = 0;
	unsigned total_count = 0;
	std::chrono::milliseconds total_age{ 0 };
	std::chrono::milliseconds max_age{ 0 };

	for (auto const & election : active_elections)
	{
		total_count++;
		auto age = election->age ();
		total_age += age;
		if (age > max_age)
		{
			max_age = age;
		}
		switch (election->behavior ())
		{
			case election_behavior::manual:
				manual_count++;
				break;
			case election_behavior::priority:
				priority_count++;
				break;
			case election_behavior::hinted:
				hinted_count++;
				break;
			case election_behavior::optimistic:
				optimistic_count++;
				break;
		}
	}
	auto average_election_age = std::chrono::milliseconds{ total_count ? total_age.count () / total_count : 0 };

	auto utilization_percentage = (static_cast<double> (total_count * 100) / node.config->active_elections.size);
	std::stringstream stream_utilization, stream_average_age;
	stream_utilization << std::fixed << std::setprecision (2) << utilization_percentage;

	response_l.put ("manual", manual_count);
	response_l.put ("priority", priority_count);
	response_l.put ("hinted", hinted_count);
	response_l.put ("optimistic", optimistic_count);
	response_l.put ("total", total_count);
	response_l.put ("aec_utilization_percentage", stream_utilization.str ());
	response_l.put ("max_election_age", max_age.count ());
	response_l.put ("average_election_age", average_election_age.count ());

	response_errors ();
}

void nano::json_handler::confirmation_history ()
{
	boost::property_tree::ptree elections;
	boost::property_tree::ptree confirmation_stats;
	std::chrono::milliseconds running_total (0);
	nano::block_hash hash (0);
	boost::optional<std::string> hash_text (request.get_optional<std::string> ("hash"));
	if (hash_text.is_initialized ())
	{
		hash = hash_impl ();
	}
	if (!ec)
	{
		for (auto const & status : node.active.recently_cemented_list ())
		{
			if (hash.is_zero () || status.get_winner ()->hash () == hash)
			{
				boost::property_tree::ptree election;
				election.put ("hash", status.get_winner ()->hash ().to_string ());
				election.put ("duration", status.get_election_duration ().count ());
				election.put ("time", status.get_election_end ().count ());
				election.put ("tally", status.get_tally ().to_string_dec ());
				election.add ("final", status.get_final_tally ().to_string_dec ());
				election.put ("blocks", std::to_string (status.get_block_count ()));
				election.put ("voters", std::to_string (status.get_voter_count ()));
				election.put ("request_count", std::to_string (status.get_confirmation_request_count ()));
				elections.push_back (std::make_pair ("", election));
			}
			running_total += status.get_election_duration ();
		}
	}
	confirmation_stats.put ("count", elections.size ());
	if (elections.size () >= 1)
	{
		confirmation_stats.put ("average", (running_total.count ()) / elections.size ());
	}
	response_l.add_child ("confirmation_stats", confirmation_stats);
	response_l.add_child ("confirmations", elections);
	response_errors ();
}

void nano::json_handler::confirmation_info ()
{
	bool const representatives = request.get<bool> ("representatives", false);
	bool const contents = request.get<bool> ("contents", true);
	bool const json_block_l = request.get<bool> ("json_block", false);
	std::string root_text (request.get<std::string> ("root"));
	nano::qualified_root root;
	if (!root.decode_hex (root_text))
	{
		auto election (node.active.election (root));
		if (election != nullptr && !node.active.confirmed (*election))
		{
			auto info = node.active.current_status (*election);
			response_l.put ("announcements", std::to_string (info.status.get_confirmation_request_count ()));
			response_l.put ("voters", std::to_string (info.votes.size ()));
			response_l.put ("last_winner", info.status.get_winner ()->hash ().to_string ());
			nano::uint128_t total (0);
			boost::property_tree::ptree blocks;
			for (auto const & [tally, block] : info.tally)
			{
				boost::property_tree::ptree entry;
				entry.put ("tally", tally.convert_to<std::string> ());
				total += tally;
				if (contents)
				{
					if (json_block_l)
					{
						boost::property_tree::ptree block_node_l;
						block->serialize_json (block_node_l);
						entry.add_child ("contents", block_node_l);
					}
					else
					{
						std::string contents;
						block->serialize_json (contents);
						entry.put ("contents", contents);
					}
				}
				if (representatives)
				{
					std::multimap<nano::uint128_t, nano::account, std::greater<nano::uint128_t>> representatives;
					for (auto const & [representative, vote] : info.votes)
					{
						if (block->hash () == vote.get_hash ())
						{
							auto amount{ node.get_rep_weight (representative) };
							representatives.emplace (std::move (amount.number ()), representative);
						}
					}
					boost::property_tree::ptree representatives_list;
					for (auto const & [amount, representative] : representatives)
					{
						representatives_list.put (representative.to_account (), amount.convert_to<std::string> ());
					}
					entry.add_child ("representatives", representatives_list);
				}
				blocks.add_child ((block->hash ()).to_string (), entry);
			}
			response_l.put ("total_tally", total.convert_to<std::string> ());
			response_l.put ("final_tally", info.status.get_final_tally ().to_string_dec ());
			response_l.add_child ("blocks", blocks);
		}
		else
		{
			ec = nano::error_rpc::confirmation_not_found;
		}
	}
	else
	{
		ec = nano::error_rpc::invalid_root;
	}
	response_errors ();
}

void nano::json_handler::confirmation_quorum ()
{
	auto quorum{ node.quorum () };

	response_l.put ("quorum_delta", quorum.quorum_delta.to_string_dec ());
	response_l.put ("online_weight_quorum_percent", std::to_string (quorum.online_weight_quorum_percent));
	response_l.put ("online_weight_minimum", quorum.online_weight_minimum.to_string_dec ());
	response_l.put ("online_stake_total", quorum.online_weight.to_string_dec ());
	response_l.put ("trended_stake_total", quorum.trended_weight.to_string_dec ());
	response_l.put ("peers_stake_total", quorum.peers_weight.to_string_dec ());
	if (request.get<bool> ("peer_details", false))
	{
		auto details = rsnano::rsn_node_representative_details (node.handle);
		auto len = rsnano::rsn_rep_details_len (details);
		boost::property_tree::ptree peers;
		for (auto i = 0; i < len; ++i)
		{
			nano::account account;
			nano::amount weight;
			rsnano::EndpointDto endpoint;
			rsnano::rsn_rep_details_get (details, i, account.bytes.data (), &endpoint, weight.bytes.data ());
			auto ep = rsnano::dto_to_endpoint (endpoint);
			auto ep_str = boost::str (boost::format ("%1%") % ep);

			boost::property_tree::ptree peer_node;
			peer_node.put ("account", account.to_account ());
			peer_node.put ("ip", ep_str);
			peer_node.put ("weight", weight.to_string_dec ());
			peers.push_back (std::make_pair ("", peer_node));
		}
		response_l.add_child ("peers", peers);
		rsnano::rsn_rep_details_destroy (details);
	}
	response_errors ();
}

void nano::json_handler::database_txn_tracker ()
{
	boost::property_tree::ptree json;

	if (node.config->diagnostics_config.txn_tracking.enable)
	{
		unsigned min_read_time_milliseconds = 0;
		boost::optional<std::string> min_read_time_text (request.get_optional<std::string> ("min_read_time"));
		if (min_read_time_text.is_initialized ())
		{
			auto success = boost::conversion::try_lexical_convert<unsigned> (*min_read_time_text, min_read_time_milliseconds);
			if (!success)
			{
				ec = nano::error_common::invalid_amount;
			}
		}

		unsigned min_write_time_milliseconds = 0;
		if (!ec)
		{
			boost::optional<std::string> min_write_time_text (request.get_optional<std::string> ("min_write_time"));
			if (min_write_time_text.is_initialized ())
			{
				auto success = boost::conversion::try_lexical_convert<unsigned> (*min_write_time_text, min_write_time_milliseconds);
				if (!success)
				{
					ec = nano::error_common::invalid_amount;
				}
			}
		}

		if (!ec)
		{
			node.store.serialize_mdb_tracker (json, std::chrono::milliseconds (min_read_time_milliseconds), std::chrono::milliseconds (min_write_time_milliseconds));
			response_l.put_child ("txn_tracking", json);
		}
	}
	else
	{
		ec = nano::error_common::tracking_not_enabled;
	}

	response_errors ();
}

void nano::json_handler::delegators ()
{
	auto representative (account_impl ());
	auto count (count_optional_impl (1024));
	auto threshold (threshold_optional_impl ());
	auto start_account_text (request.get_optional<std::string> ("start"));

	nano::account start_account{};
	if (!ec && start_account_text.is_initialized ())
	{
		start_account = account_impl (start_account_text.get ());
	}

	if (!ec)
	{
		auto transaction (node.store.tx_begin_read ());
		boost::property_tree::ptree delegators;
		for (auto i (node.store.account ().begin (*transaction, start_account.number () + 1)), n (node.store.account ().end ()); i != n && delegators.size () < count; ++i)
		{
			nano::account_info const & info (i->second);
			if (info.representative () == representative)
			{
				if (info.balance ().number () >= threshold.number ())
				{
					std::string balance;
					nano::uint128_union (info.balance ()).encode_dec (balance);
					nano::account const & delegator (i->first);
					delegators.put (delegator.to_account (), balance);
				}
			}
		}
		response_l.add_child ("delegators", delegators);
	}
	response_errors ();
}
