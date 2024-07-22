#include "nano/lib/rsnano.hpp"
#include <nano/lib/blocks.hpp>
#include <nano/lib/config.hpp>
#include <nano/lib/json_error_response.hpp>
#include <nano/lib/timer.hpp>
#include <nano/node/active_elections.hpp>
#include <nano/node/bootstrap/bootstrap_lazy.hpp>
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
			auto existed (node.bootstrap_initiator.current_lazy_attempt () != nullptr);
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
	rsnano::ConfirmationQuorumDto quorum;
	rsnano::rsn_node_confirmation_quorum(node.handle, &quorum);
	auto delta{nano::amount::from_bytes(quorum.quorum_delta)};
	auto minimum{nano::amount::from_bytes(quorum.online_weight_minimum)};
	auto online{nano::amount::from_bytes(quorum.online_weight)};
	auto trended{nano::amount::from_bytes(quorum.trended_weight)};
	auto peers{nano::amount::from_bytes(quorum.peers_weight)};

	response_l.put ("quorum_delta", delta.to_string_dec());
	response_l.put ("online_weight_quorum_percent", std::to_string (quorum.online_weight_quorum_percent));
	response_l.put ("online_weight_minimum", minimum.to_string_dec ());
	response_l.put ("online_stake_total", online.to_string_dec());
	response_l.put ("trended_stake_total", trended.to_string_dec());
	response_l.put ("peers_stake_total", peers.to_string_dec());
	if (request.get<bool> ("peer_details", false))
	{
		boost::property_tree::ptree peers;
		for (auto & peer : node.representative_register.representatives ())
		{
			boost::property_tree::ptree peer_node;
			peer_node.put ("account", peer.get_account ().to_account ());
			peer_node.put ("ip", peer.get_channel ()->to_string ());
			peer_node.put ("weight", nano::amount{ node.ledger.weight (peer.get_account ()) }.to_string_dec ());
			peers.push_back (std::make_pair ("", peer_node));
		}
		response_l.add_child ("peers", peers);
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

void nano::json_handler::delegators_count ()
{
	auto account (account_impl ());
	if (!ec)
	{
		uint64_t count (0);
		auto transaction (node.store.tx_begin_read ());
		for (auto i (node.store.account ().begin (*transaction)), n (node.store.account ().end ()); i != n; ++i)
		{
			nano::account_info const & info (i->second);
			if (info.representative () == account)
			{
				++count;
			}
		}
		response_l.put ("count", std::to_string (count));
	}
	response_errors ();
}

void nano::json_handler::deterministic_key ()
{
	std::string seed_text (request.get<std::string> ("seed"));
	std::string index_text (request.get<std::string> ("index"));
	nano::raw_key seed;
	if (!seed.decode_hex (seed_text))
	{
		try
		{
			uint32_t index (std::stoul (index_text));
			nano::raw_key prv = nano::deterministic_key (seed, index);
			nano::public_key pub (nano::pub_key (prv));
			response_l.put ("private", prv.to_string ());
			response_l.put ("public", pub.to_string ());
			response_l.put ("account", pub.to_account ());
		}
		catch (std::logic_error const &)
		{
			ec = nano::error_common::invalid_index;
		}
	}
	else
	{
		ec = nano::error_common::bad_seed;
	}
	response_errors ();
}

void nano::json_handler::frontiers ()
{
	auto start (account_impl ());
	auto count (count_impl ());
	if (!ec)
	{
		boost::property_tree::ptree frontiers;
		auto transaction (node.store.tx_begin_read ());
		for (auto i (node.store.account ().begin (*transaction, start)), n (node.store.account ().end ()); i != n && frontiers.size () < count; ++i)
		{
			frontiers.put (i->first.to_account (), i->second.head ().to_string ());
		}
		response_l.add_child ("frontiers", frontiers);
	}
	response_errors ();
}

void nano::json_handler::account_count ()
{
	auto size (node.ledger.account_count ());
	response_l.put ("count", std::to_string (size));
	response_errors ();
}

namespace
{
class history_visitor : public nano::block_visitor
{
public:
	history_visitor (nano::json_handler & handler_a, bool raw_a, nano::store::transaction & transaction_a, boost::property_tree::ptree & tree_a, nano::block_hash const & hash_a, std::vector<nano::public_key> const & accounts_filter_a) :
		handler (handler_a),
		raw (raw_a),
		transaction (transaction_a),
		tree (tree_a),
		hash (hash_a),
		accounts_filter (accounts_filter_a)
	{
	}
	virtual ~history_visitor () = default;
	void send_block (nano::send_block const & block_a)
	{
		if (should_ignore_account (block_a.destination_field ().value ()))
		{
			return;
		}
		tree.put ("type", "send");
		auto account (block_a.destination_field ().value ().to_account ());
		tree.put ("account", account);
		auto amount = handler.node.ledger.any ().block_amount (transaction, hash);
		if (amount)
		{
			tree.put ("amount", amount.value ().number ().convert_to<std::string> ());
		}
		if (raw)
		{
			tree.put ("destination", account);
			tree.put ("balance", block_a.balance ().to_string_dec ());
			tree.put ("previous", block_a.previous ().to_string ());
		}
	}
	void receive_block (nano::receive_block const & block_a)
	{
		tree.put ("type", "receive");
		auto amount = handler.node.ledger.any ().block_amount (transaction, hash);
		if (amount)
		{
			auto source_account = handler.node.ledger.any ().block_account (transaction, block_a.source_field ().value ());
			if (source_account)
			{
				tree.put ("account", source_account.value ().to_account ());
			}
			tree.put ("amount", amount.value ().number ().convert_to<std::string> ());
		}
		if (raw)
		{
			tree.put ("source", block_a.source_field ().value ().to_string ());
			tree.put ("previous", block_a.previous ().to_string ());
		}
	}
	void open_block (nano::open_block const & block_a)
	{
		if (raw)
		{
			tree.put ("type", "open");
			tree.put ("representative", block_a.representative_field ().value ().to_account ());
			tree.put ("source", block_a.source_field ().value ().to_string ());
			tree.put ("opened", block_a.account ().to_account ());
		}
		else
		{
			// Report opens as a receive
			tree.put ("type", "receive");
		}
		if (block_a.source () != handler.node.ledger.constants.genesis->account_field ())
		{
			auto amount = handler.node.ledger.any ().block_amount (transaction, hash);
			if (amount)
			{
				auto source_account (handler.node.ledger.any ().block_account (transaction, block_a.source_field ().value ()));
				if (source_account)
				{
					tree.put ("account", source_account.value ().to_account ());
				}
				tree.put ("amount", amount.value ().number ().convert_to<std::string> ());
			}
		}
		else
		{
			tree.put ("account", handler.node.ledger.constants.genesis->account_field ().value ().to_account ());
			tree.put ("amount", nano::dev::constants.genesis_amount.convert_to<std::string> ());
		}
	}
	void change_block (nano::change_block const & block_a)
	{
		if (raw && accounts_filter.empty ())
		{
			tree.put ("type", "change");
			tree.put ("representative", block_a.representative_field ().value ().to_account ());
			tree.put ("previous", block_a.previous ().to_string ());
		}
	}
	void state_block (nano::state_block const & block_a)
	{
		if (raw)
		{
			tree.put ("type", "state");
			tree.put ("representative", block_a.representative_field ().value ().to_account ());
			tree.put ("link", block_a.link_field ().value ().to_string ());
			tree.put ("balance", block_a.balance ().to_string_dec ());
			tree.put ("previous", block_a.previous ().to_string ());
		}
		auto balance (block_a.balance ().number ());
		auto previous_balance = handler.node.ledger.any ().block_balance (transaction, block_a.previous ());
		if (!previous_balance)
		{
			if (raw)
			{
				tree.put ("subtype", "unknown");
			}
			else
			{
				tree.put ("type", "unknown");
			}
		}
		else if (balance < previous_balance.value ().number ())
		{
			if (should_ignore_account (block_a.link_field ().value ().as_account ()))
			{
				tree.clear ();
				return;
			}
			if (raw)
			{
				tree.put ("subtype", "send");
			}
			else
			{
				tree.put ("type", "send");
			}
			tree.put ("account", block_a.link_field ().value ().to_account ());
			tree.put ("amount", (previous_balance.value ().number () - balance).convert_to<std::string> ());
		}
		else
		{
			if (block_a.link_field ().value ().is_zero ())
			{
				if (raw && accounts_filter.empty ())
				{
					tree.put ("subtype", "change");
				}
			}
			else if (balance == previous_balance.value ().number () && handler.node.ledger.is_epoch_link (block_a.link_field ().value ()))
			{
				if (raw && accounts_filter.empty ())
				{
					tree.put ("subtype", "epoch");
					tree.put ("account", handler.node.ledger.epoch_signer (block_a.link_field ().value ()).to_account ());
				}
			}
			else
			{
				auto source_account = handler.node.ledger.any ().block_account (transaction, block_a.link_field ().value ().as_block_hash ());
				if (source_account && should_ignore_account (source_account.value ()))
				{
					tree.clear ();
					return;
				}
				if (raw)
				{
					tree.put ("subtype", "receive");
				}
				else
				{
					tree.put ("type", "receive");
				}
				if (source_account)
				{
					tree.put ("account", source_account.value ().to_account ());
				}
				tree.put ("amount", (balance - previous_balance.value ().number ()).convert_to<std::string> ());
			}
		}
	}
	bool should_ignore_account (nano::public_key const & account)
	{
		bool ignore (false);
		if (!accounts_filter.empty ())
		{
			if (std::find (accounts_filter.begin (), accounts_filter.end (), account) == accounts_filter.end ())
			{
				ignore = true;
			}
		}
		return ignore;
	}
	nano::json_handler & handler;
	bool raw;
	nano::store::transaction & transaction;
	boost::property_tree::ptree & tree;
	nano::block_hash const & hash;
	std::vector<nano::public_key> const & accounts_filter;
};
}

void nano::json_handler::account_history ()
{
	std::vector<nano::public_key> accounts_to_filter;
	auto const accounts_filter_node = request.get_child_optional ("account_filter");
	if (accounts_filter_node.is_initialized ())
	{
		for (auto & a : (*accounts_filter_node))
		{
			auto account (account_impl (a.second.get<std::string> ("")));
			if (!ec)
			{
				accounts_to_filter.push_back (account);
			}
			else
			{
				break;
			}
		}
	}
	nano::account account;
	nano::block_hash hash;
	bool reverse (request.get_optional<bool> ("reverse") == true);
	auto head_str (request.get_optional<std::string> ("head"));
	auto transaction (node.store.tx_begin_read ());
	auto count (count_impl ());
	auto offset (offset_optional_impl (0));
	if (head_str)
	{
		if (!hash.decode_hex (*head_str))
		{
			if (node.ledger.any ().block_exists (*transaction, hash))
			{
				account = node.ledger.any ().block_account (*transaction, hash).value ();
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
	else
	{
		account = account_impl ();
		if (!ec)
		{
			if (reverse)
			{
				auto info (account_info_impl (*transaction, account));
				if (!ec)
				{
					hash = info.open_block ();
				}
			}
			else
			{
				hash = node.ledger.any ().account_head (*transaction, account);
			}
		}
	}
	if (!ec)
	{
		boost::property_tree::ptree history;
		bool output_raw (request.get_optional<bool> ("raw") == true);
		response_l.put ("account", account.to_account ());
		auto block (node.ledger.any ().block_get (*transaction, hash));
		while (block != nullptr && count > 0)
		{
			if (offset > 0)
			{
				--offset;
			}
			else
			{
				boost::property_tree::ptree entry;
				history_visitor visitor (*this, output_raw, *transaction, entry, hash, accounts_to_filter);
				block->visit (visitor);
				if (!entry.empty ())
				{
					entry.put ("local_timestamp", std::to_string (block->sideband ().timestamp ()));
					entry.put ("height", std::to_string (block->sideband ().height ()));
					entry.put ("hash", hash.to_string ());
					entry.put ("confirmed", node.ledger.confirmed ().block_exists_or_pruned (*transaction, hash));
					if (output_raw)
					{
						entry.put ("work", nano::to_string_hex (block->block_work ()));
						entry.put ("signature", block->block_signature ().to_string ());
					}
					history.push_back (std::make_pair ("", entry));
					--count;
				}
			}
			hash = reverse ? node.ledger.any ().block_successor (*transaction, hash).value_or (0) : block->previous ();
			block = node.ledger.any ().block_get (*transaction, hash);
		}
		response_l.add_child ("history", history);
		if (!hash.is_zero ())
		{
			response_l.put (reverse ? "next" : "previous", hash.to_string ());
		}
	}
	response_errors ();
}

void nano::json_handler::keepalive ()
{
	if (!ec)
	{
		std::string address_text (request.get<std::string> ("address"));
		std::string port_text (request.get<std::string> ("port"));
		uint16_t port;
		if (!nano::parse_port (port_text, port))
		{
			node.keepalive (address_text, port);
			response_l.put ("started", "1");
		}
		else
		{
			ec = nano::error_common::invalid_port;
		}
	}
	response_errors ();
}

void nano::json_handler::key_create ()
{
	nano::keypair pair;
	response_l.put ("private", pair.prv.to_string ());
	response_l.put ("public", pair.pub.to_string ());
	response_l.put ("account", pair.pub.to_account ());
	response_errors ();
}

void nano::json_handler::key_expand ()
{
	std::string key_text (request.get<std::string> ("key"));
	nano::raw_key prv;
	if (!prv.decode_hex (key_text))
	{
		nano::public_key pub (nano::pub_key (prv));
		response_l.put ("private", prv.to_string ());
		response_l.put ("public", pub.to_string ());
		response_l.put ("account", pub.to_account ());
	}
	else
	{
		ec = nano::error_common::bad_private_key;
	}
	response_errors ();
}

void nano::json_handler::ledger ()
{
	auto count (count_optional_impl ());
	auto threshold (threshold_optional_impl ());
	if (!ec)
	{
		nano::account start{};
		boost::optional<std::string> account_text (request.get_optional<std::string> ("account"));
		if (account_text.is_initialized ())
		{
			start = account_impl (account_text.get ());
		}
		uint64_t modified_since (0);
		boost::optional<std::string> modified_since_text (request.get_optional<std::string> ("modified_since"));
		if (modified_since_text.is_initialized ())
		{
			if (decode_unsigned (modified_since_text.get (), modified_since))
			{
				ec = nano::error_rpc::invalid_timestamp;
			}
		}
		bool const sorting = request.get<bool> ("sorting", false);
		bool const representative = request.get<bool> ("representative", false);
		bool const weight = request.get<bool> ("weight", false);
		bool const pending = request.get<bool> ("pending", false);
		bool const receivable = request.get<bool> ("receivable", pending);
		boost::property_tree::ptree accounts;
		auto transaction (node.store.tx_begin_read ());
		if (!ec && !sorting) // Simple
		{
			for (auto i (node.store.account ().begin (*transaction, start)), n (node.store.account ().end ()); i != n && accounts.size () < count; ++i)
			{
				nano::account_info const & info (i->second);
				if (info.modified () >= modified_since && (receivable || info.balance ().number () >= threshold.number ()))
				{
					nano::account const & account (i->first);
					boost::property_tree::ptree response_a;
					if (receivable)
					{
						auto account_receivable = node.ledger.account_receivable (*transaction, account);
						if (info.balance ().number () + account_receivable < threshold.number ())
						{
							continue;
						}
						response_a.put ("pending", account_receivable.convert_to<std::string> ());
						response_a.put ("receivable", account_receivable.convert_to<std::string> ());
					}
					response_a.put ("frontier", info.head ().to_string ());
					response_a.put ("open_block", info.open_block ().to_string ());
					response_a.put ("representative_block", node.ledger.representative (*transaction, info.head ()).to_string ());
					std::string balance;
					nano::uint128_union (info.balance ()).encode_dec (balance);
					response_a.put ("balance", balance);
					response_a.put ("modified_timestamp", std::to_string (info.modified ()));
					response_a.put ("block_count", std::to_string (info.block_count ()));
					if (representative)
					{
						response_a.put ("representative", info.representative ().to_account ());
					}
					if (weight)
					{
						auto account_weight (node.ledger.weight_exact (*transaction, account));
						response_a.put ("weight", account_weight.convert_to<std::string> ());
					}
					accounts.push_back (std::make_pair (account.to_account (), response_a));
				}
			}
		}
		else if (!ec) // Sorting
		{
			std::vector<std::pair<nano::uint128_union, nano::account>> ledger_l;
			for (auto i (node.store.account ().begin (*transaction, start)), n (node.store.account ().end ()); i != n; ++i)
			{
				nano::account_info const & info (i->second);
				nano::uint128_union balance (info.balance ());
				if (info.modified () >= modified_since)
				{
					ledger_l.emplace_back (balance, i->first);
				}
			}
			std::sort (ledger_l.begin (), ledger_l.end ());
			std::reverse (ledger_l.begin (), ledger_l.end ());
			nano::account_info info;
			for (auto i (ledger_l.begin ()), n (ledger_l.end ()); i != n && accounts.size () < count; ++i)
			{
				node.store.account ().get (*transaction, i->second, info);
				if (receivable || info.balance ().number () >= threshold.number ())
				{
					nano::account const & account (i->second);
					boost::property_tree::ptree response_a;
					if (receivable)
					{
						auto account_receivable = node.ledger.account_receivable (*transaction, account);
						if (info.balance ().number () + account_receivable < threshold.number ())
						{
							continue;
						}
						response_a.put ("pending", account_receivable.convert_to<std::string> ());
						response_a.put ("receivable", account_receivable.convert_to<std::string> ());
					}
					response_a.put ("frontier", info.head ().to_string ());
					response_a.put ("open_block", info.open_block ().to_string ());
					response_a.put ("representative_block", node.ledger.representative (*transaction, info.head ()).to_string ());
					std::string balance;
					(i->first).encode_dec (balance);
					response_a.put ("balance", balance);
					response_a.put ("modified_timestamp", std::to_string (info.modified ()));
					response_a.put ("block_count", std::to_string (info.block_count ()));
					if (representative)
					{
						response_a.put ("representative", info.representative ().to_account ());
					}
					if (weight)
					{
						auto account_weight (node.ledger.weight_exact (*transaction, account));
						response_a.put ("weight", account_weight.convert_to<std::string> ());
					}
					accounts.push_back (std::make_pair (account.to_account (), response_a));
				}
			}
		}
		response_l.add_child ("accounts", accounts);
	}
	response_errors ();
}

void nano::json_handler::mnano_from_raw (nano::uint128_t ratio)
{
	auto amount (amount_impl ());
	response_l.put ("deprecated", "1");
	if (!ec)
	{
		auto result (amount.number () / ratio);
		response_l.put ("amount", result.convert_to<std::string> ());
	}
	response_errors ();
}

void nano::json_handler::mnano_to_raw (nano::uint128_t ratio)
{
	auto amount (amount_impl ());
	response_l.put ("deprecated", "1");
	if (!ec)
	{
		auto result (amount.number () * ratio);
		if (result > amount.number ())
		{
			response_l.put ("amount", result.convert_to<std::string> ());
		}
		else
		{
			ec = nano::error_common::invalid_amount_big;
		}
	}
	response_errors ();
}

void nano::json_handler::nano_to_raw ()
{
	auto amount (amount_impl ());
	if (!ec)
	{
		auto result (amount.number () * nano::Mxrb_ratio);
		if (result > amount.number ())
		{
			response_l.put ("amount", result.convert_to<std::string> ());
		}
		else
		{
			ec = nano::error_common::invalid_amount_big;
		}
	}
	response_errors ();
}

void nano::json_handler::raw_to_nano ()
{
	auto amount (amount_impl ());
	if (!ec)
	{
		auto result (amount.number () / nano::Mxrb_ratio);
		response_l.put ("amount", result.convert_to<std::string> ());
	}
	response_errors ();
}

/*
 * @warning This is an internal/diagnostic RPC, do not rely on its interface being stable
 */
void nano::json_handler::node_id ()
{
	if (!ec)
	{
		response_l.put ("public", node.node_id.pub.to_string ());
		response_l.put ("as_account", node.node_id.pub.to_account ());
		response_l.put ("node_id", node.node_id.pub.to_node_id ());
	}
	response_errors ();
}

/*
 * @warning This is an internal/diagnostic RPC, do not rely on its interface being stable
 */
void nano::json_handler::node_id_delete ()
{
	response_l.put ("deprecated", "1");
	response_errors ();
}

void nano::json_handler::password_change ()
{
	node.workers->push_task (create_worker_task ([] (std::shared_ptr<nano::json_handler> const & rpc_l) {
		auto wallet_id (rpc_l->get_wallet_id ());
		if (!rpc_l->ec)
		{
			std::string password_text (rpc_l->request.get<std::string> ("password"));

			auto error = rpc_l->node.wallets.rekey (wallet_id, password_text);
			rpc_l->response_l.put ("changed", error == nano::wallets_error::none ? "1" : "0");
			rpc_l->set_error (error);
			if (error == nano::wallets_error::none)
			{
				rpc_l->node.logger->warn (nano::log::type::rpc, "Wallet password changed");
			}
		}
		rpc_l->response_errors ();
	}));
}

void nano::json_handler::password_enter ()
{
	node.workers->push_task (create_worker_task ([&wallets = node.wallets] (std::shared_ptr<nano::json_handler> const & rpc_l) {
		auto wallet_id{ rpc_l->get_wallet_id () };
		if (!rpc_l->ec)
		{
			std::string password_text (rpc_l->request.get<std::string> ("password"));
			auto error = wallets.enter_password (wallet_id, password_text);
			if (error == nano::wallets_error::none)
			{
				rpc_l->response_l.put ("valid", "1");
			}
			else if (error == nano::wallets_error::invalid_password)
			{
				rpc_l->response_l.put ("valid", "0");
			}
			else
			{
				rpc_l->set_error (error);
			}
		}
		rpc_l->response_errors ();
	}));
}

void nano::json_handler::password_valid (bool wallet_locked)
{
	auto wallet_id (get_wallet_id ());
	if (!ec)
	{
		bool valid = false;
		auto error = node.wallets.valid_password (wallet_id, valid);
		if (error == nano::wallets_error::none)
		{
			if (!wallet_locked)
			{
				response_l.put ("valid", valid ? "1" : "0");
			}
			else
			{
				response_l.put ("locked", valid ? "0" : "1");
			}
		}
		else
		{
			set_error (error);
		}
	}
	response_errors ();
}

void nano::json_handler::peers ()
{
	boost::property_tree::ptree peers_l;
	bool const peer_details = request.get<bool> ("peer_details", false);
	auto peers_list (node.network->tcp_channels->list (std::numeric_limits<std::size_t>::max ()));
	std::sort (peers_list.begin (), peers_list.end (), [] (auto const & lhs, auto const & rhs) {
		return lhs->get_remote_endpoint () < rhs->get_remote_endpoint ();
	});
	for (auto i (peers_list.begin ()), n (peers_list.end ()); i != n; ++i)
	{
		std::stringstream text;
		auto channel (*i);
		text << channel->to_string ();
		if (peer_details)
		{
			boost::property_tree::ptree pending_tree;
			pending_tree.put ("protocol_version", std::to_string (channel->get_network_version ()));
			auto node_id_l (channel->get_node_id_optional ());
			if (node_id_l.is_initialized ())
			{
				pending_tree.put ("node_id", node_id_l.get ().to_node_id ());
			}
			else
			{
				pending_tree.put ("node_id", "");
			}
			debug_assert (channel->get_type () == nano::transport::transport_type::tcp);
			pending_tree.put ("type", "tcp");

			auto peering_endpoint = channel->get_peering_endpoint ();
			pending_tree.put ("peering", boost::lexical_cast<std::string> (peering_endpoint));

			peers_l.push_back (boost::property_tree::ptree::value_type (text.str (), pending_tree));
		}
		else
		{
			peers_l.push_back (boost::property_tree::ptree::value_type (text.str (), boost::property_tree::ptree (std::to_string (channel->get_network_version ()))));
		}
	}
	response_l.add_child ("peers", peers_l);
	response_errors ();
}

void nano::json_handler::pending ()
{
	response_l.put ("deprecated", "1");
	receivable ();
}

void nano::json_handler::receivable ()
{
	auto account (account_impl ());
	auto count (count_optional_impl ());
	auto offset (offset_optional_impl (0));
	auto threshold (threshold_optional_impl ());
	bool const source = request.get<bool> ("source", false);
	bool const min_version = request.get<bool> ("min_version", false);
	bool const include_active = request.get<bool> ("include_active", false);
	bool const include_only_confirmed = request.get<bool> ("include_only_confirmed", true);
	bool const sorting = request.get<bool> ("sorting", false);
	auto simple (threshold.is_zero () && !source && !min_version && !sorting); // if simple, response is a list of hashes
	bool const should_sort = sorting && !simple;
	if (!ec)
	{
		auto offset_counter = offset;
		boost::property_tree::ptree peers_l;
		auto transaction = node.store.tx_begin_read ();
		// The ptree container is used if there are any children nodes (e.g source/min_version) otherwise the amount container is used.
		std::vector<std::pair<std::string, boost::property_tree::ptree>> hash_ptree_pairs;
		std::vector<std::pair<std::string, nano::uint128_t>> hash_amount_pairs;
		for (auto current = node.ledger.any ().receivable_upper_bound (*transaction, account, 0); !current.is_end () && (should_sort || peers_l.size () < count); ++current)
		{
			auto const & [key, info] = *current;
			if (include_only_confirmed && !node.ledger.confirmed ().block_exists_or_pruned (*transaction, key.hash))
			{
				continue;
			}
			if (!should_sort && offset_counter > 0)
			{
				--offset_counter;
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
			if (source || min_version)
			{
				boost::property_tree::ptree pending_tree;
				pending_tree.put ("amount", info.amount.number ().convert_to<std::string> ());
				if (source)
				{
					pending_tree.put ("source", info.source.to_account ());
				}
				if (min_version)
				{
					pending_tree.put ("min_version", epoch_as_string (info.epoch));
				}

				if (should_sort)
				{
					hash_ptree_pairs.emplace_back (key.hash.to_string (), pending_tree);
				}
				else
				{
					peers_l.add_child (key.hash.to_string (), pending_tree);
				}
			}
			else
			{
				if (should_sort)
				{
					hash_amount_pairs.emplace_back (key.hash.to_string (), info.amount.number ());
				}
				else
				{
					peers_l.put (key.hash.to_string (), info.amount.number ().convert_to<std::string> ());
				}
			}
		}
		if (should_sort)
		{
			if (source || min_version)
			{
				std::stable_sort (hash_ptree_pairs.begin (), hash_ptree_pairs.end (), [] (auto const & lhs, auto const & rhs) {
					return lhs.second.template get<nano::uint128_t> ("amount") > rhs.second.template get<nano::uint128_t> ("amount");
				});
				for (auto i = offset, j = offset + count; i < hash_ptree_pairs.size () && i < j; ++i)
				{
					peers_l.add_child (hash_ptree_pairs[i].first, hash_ptree_pairs[i].second);
				}
			}
			else
			{
				std::stable_sort (hash_amount_pairs.begin (), hash_amount_pairs.end (), [] (auto const & lhs, auto const & rhs) {
					return lhs.second > rhs.second;
				});

				for (auto i = offset, j = offset + count; i < hash_amount_pairs.size () && i < j; ++i)
				{
					peers_l.put (hash_amount_pairs[i].first, hash_amount_pairs[i].second.convert_to<std::string> ());
				}
			}
		}
		response_l.add_child ("blocks", peers_l);
	}
	response_errors ();
}

void nano::json_handler::pending_exists ()
{
	response_l.put ("deprecated", "1");
	receivable_exists ();
}

void nano::json_handler::receivable_exists ()
{
	auto hash (hash_impl ());
	bool const include_active = request.get<bool> ("include_active", false);
	bool const include_only_confirmed = request.get<bool> ("include_only_confirmed", true);
	if (!ec)
	{
		auto transaction (node.store.tx_begin_read ());
		auto block (node.ledger.any ().block_get (*transaction, hash));
		if (block != nullptr)
		{
			auto exists (false);
			if (block->is_send ())
			{
				exists = node.ledger.any ().pending_get (*transaction, nano::pending_key{ block->destination (), hash }).has_value ();
			}
			exists = exists && (block_confirmed (node, *transaction, block->hash (), include_active, include_only_confirmed));
			response_l.put ("exists", exists ? "1" : "0");
		}
		else
		{
			ec = nano::error_blocks::not_found;
		}
	}
	response_errors ();
}

void nano::json_handler::process ()
{
	node.workers->push_task (create_worker_task ([] (std::shared_ptr<nano::json_handler> const & rpc_l) {
		bool const is_async = rpc_l->request.get<bool> ("async", false);
		auto block (rpc_l->block_impl (true));

		// State blocks subtype check
		if (!rpc_l->ec && block->type () == nano::block_type::state)
		{
			std::string subtype_text (rpc_l->request.get<std::string> ("subtype", ""));
			if (!subtype_text.empty ())
			{
				std::shared_ptr<nano::state_block> block_state (std::static_pointer_cast<nano::state_block> (block));
				auto transaction (rpc_l->node.store.tx_begin_read ());
				if (!block_state->previous ().is_zero () && !rpc_l->node.ledger.any ().block_exists (*transaction, block_state->previous ()))
				{
					rpc_l->ec = nano::error_process::gap_previous;
				}
				else
				{
					auto balance (rpc_l->node.ledger.any ().account_balance (*transaction, block_state->account_field ().value ()).value_or (0).number ());
					if (subtype_text == "send")
					{
						if (balance <= block_state->balance_field ().value ().number ())
						{
							rpc_l->ec = nano::error_rpc::invalid_subtype_balance;
						}
						// Send with previous == 0 fails balance check. No previous != 0 check required
					}
					else if (subtype_text == "receive")
					{
						if (balance > block_state->balance_field ().value ().number ())
						{
							rpc_l->ec = nano::error_rpc::invalid_subtype_balance;
						}
						// Receive can be point to open block. No previous != 0 check required
					}
					else if (subtype_text == "open")
					{
						if (!block_state->previous ().is_zero ())
						{
							rpc_l->ec = nano::error_rpc::invalid_subtype_previous;
						}
					}
					else if (subtype_text == "change")
					{
						if (balance != block_state->balance_field ().value ().number ())
						{
							rpc_l->ec = nano::error_rpc::invalid_subtype_balance;
						}
						else if (block_state->previous ().is_zero ())
						{
							rpc_l->ec = nano::error_rpc::invalid_subtype_previous;
						}
					}
					else if (subtype_text == "epoch")
					{
						if (balance != block_state->balance_field ().value ().number ())
						{
							rpc_l->ec = nano::error_rpc::invalid_subtype_balance;
						}
						else if (!rpc_l->node.ledger.is_epoch_link (block_state->link_field ().value ()))
						{
							rpc_l->ec = nano::error_rpc::invalid_subtype_epoch_link;
						}
					}
					else
					{
						rpc_l->ec = nano::error_rpc::invalid_subtype;
					}
				}
			}
		}
		if (!rpc_l->ec)
		{
			if (!rpc_l->node.network_params.work.validate_entry (*block))
			{
				if (!is_async)
				{
					auto result_maybe = rpc_l->node.process_local (block);
					if (!result_maybe)
					{
						rpc_l->ec = nano::error_rpc::stopped;
					}
					else
					{
						auto const & result = result_maybe.value ();
						switch (result)
						{
							case nano::block_status::progress:
							{
								rpc_l->response_l.put ("hash", block->hash ().to_string ());
								break;
							}
							case nano::block_status::gap_previous:
							{
								rpc_l->ec = nano::error_process::gap_previous;
								break;
							}
							case nano::block_status::gap_source:
							{
								rpc_l->ec = nano::error_process::gap_source;
								break;
							}
							case nano::block_status::old:
							{
								rpc_l->ec = nano::error_process::old;
								break;
							}
							case nano::block_status::bad_signature:
							{
								rpc_l->ec = nano::error_process::bad_signature;
								break;
							}
							case nano::block_status::negative_spend:
							{
								// TODO once we get RPC versioning, this should be changed to "negative spend"
								rpc_l->ec = nano::error_process::negative_spend;
								break;
							}
							case nano::block_status::balance_mismatch:
							{
								rpc_l->ec = nano::error_process::balance_mismatch;
								break;
							}
							case nano::block_status::unreceivable:
							{
								rpc_l->ec = nano::error_process::unreceivable;
								break;
							}
							case nano::block_status::block_position:
							{
								rpc_l->ec = nano::error_process::block_position;
								break;
							}
							case nano::block_status::gap_epoch_open_pending:
							{
								rpc_l->ec = nano::error_process::gap_epoch_open_pending;
								break;
							}
							case nano::block_status::fork:
							{
								bool const force = rpc_l->request.get<bool> ("force", false);
								if (force)
								{
									rpc_l->node.active.erase (*block);
									rpc_l->node.block_processor.force (block);
									rpc_l->response_l.put ("hash", block->hash ().to_string ());
								}
								else
								{
									rpc_l->ec = nano::error_process::fork;
								}
								break;
							}
							case nano::block_status::insufficient_work:
							{
								rpc_l->ec = nano::error_process::insufficient_work;
								break;
							}
							case nano::block_status::opened_burn_account:
								rpc_l->ec = nano::error_process::opened_burn_account;
								break;
							default:
							{
								rpc_l->ec = nano::error_process::other;
								break;
							}
						}
					}
				}
				else
				{
					if (block->type () == nano::block_type::state)
					{
						rpc_l->node.process_local_async (block);
						rpc_l->response_l.put ("started", "1");
					}
					else
					{
						rpc_l->ec = nano::error_common::is_not_state_block;
					}
				}
			}
			else
			{
				rpc_l->ec = nano::error_blocks::work_low;
			}
		}
		rpc_l->response_errors ();
	}));
}

void nano::json_handler::pruned_exists ()
{
	auto hash (hash_impl ());
	if (!ec)
	{
		auto transaction (node.store.tx_begin_read ());
		if (node.ledger.pruning_enabled ())
		{
			auto exists (node.store.pruned ().exists (*transaction, hash));
			response_l.put ("exists", exists ? "1" : "0");
		}
		else
		{
			ec = nano::error_rpc::pruning_disabled;
		}
	}
	response_errors ();
}

void nano::json_handler::receive ()
{
	auto wallet_id{ get_wallet_id () };
	auto account (account_impl ());
	auto hash (hash_impl ("block"));
	if (!ec)
	{
		auto block_transaction (node.store.tx_begin_read ());
		if (node.ledger.any ().block_exists_or_pruned (*block_transaction, hash))
		{
			auto pending_info = node.ledger.any ().pending_get (*block_transaction, nano::pending_key (account, hash));
			if (pending_info)
			{
				auto work (work_optional_impl ());
				if (!ec && work)
				{
					nano::root head;
					nano::epoch epoch = pending_info->epoch;
					auto info = node.ledger.any ().account_get (*block_transaction, account);
					if (info)
					{
						head = info->head ();
						// When receiving, epoch version is the higher between the previous and the source blocks
						epoch = std::max (info->epoch (), epoch);
					}
					else
					{
						head = account;
					}
					nano::block_details details (epoch, false, true, false);
					if (node.network_params.work.difficulty (nano::work_version::work_1, head, work) < node.network_params.work.threshold (nano::work_version::work_1, details))
					{
						ec = nano::error_common::invalid_work;
					}
				}
				else if (!ec) // && work == 0
				{
					if (!node.work_generation_enabled ())
					{
						ec = nano::error_common::disabled_work_generation;
					}
				}
				if (!ec)
				{
					// Representative is only used by receive_action when opening accounts
					// Set a wallet default representative for new accounts
					nano::account representative;
					auto error = node.wallets.get_representative (wallet_id, representative);
					set_error (error);
					if (error == nano::wallets_error::none)
					{
						bool generate_work (work == 0); // Disable work generation if "work" option is provided
						auto response_a (response);
						error = node.wallets.receive_async (
						wallet_id,
						hash, representative, nano::dev::constants.genesis_amount, account, [response_a] (std::shared_ptr<nano::block> const & block_a) {
							if (block_a != nullptr)
							{
								boost::property_tree::ptree response_l;
								response_l.put ("block", block_a->hash ().to_string ());
								std::stringstream ostream;
								boost::property_tree::write_json (ostream, response_l);
								response_a (ostream.str ());
							}
							else
							{
								json_error_response (response_a, "Error generating block");
							}
						},
						work, generate_work);
					}
				}
			}
			else
			{
				ec = nano::error_process::unreceivable;
			}
		}
		else
		{
			ec = nano::error_blocks::not_found;
		}
	}
	// Because of receive_async
	if (ec)
	{
		response_errors ();
	}
}

void nano::json_handler::receive_minimum ()
{
	if (!ec)
	{
		response_l.put ("amount", node.config->receive_minimum.to_string_dec ());
	}
	response_errors ();
}

void nano::json_handler::receive_minimum_set ()
{
	auto amount (amount_impl ());
	if (!ec)
	{
		node.config->receive_minimum = amount;
		response_l.put ("success", "");
	}
	response_errors ();
}

void nano::json_handler::representatives ()
{
	auto count (count_optional_impl ());
	if (!ec)
	{
		bool const sorting = request.get<bool> ("sorting", false);
		boost::property_tree::ptree representatives;
		auto rep_amounts = node.get_rep_weights ();
		if (!sorting) // Simple
		{
			std::map<nano::account, nano::uint128_t> ordered (rep_amounts.begin (), rep_amounts.end ());
			for (auto & rep_amount : rep_amounts)
			{
				auto const & account (rep_amount.first);
				auto const & amount (rep_amount.second);
				representatives.put (account.to_account (), amount.convert_to<std::string> ());

				if (representatives.size () > count)
				{
					break;
				}
			}
		}
		else // Sorting
		{
			std::vector<std::pair<nano::uint128_t, std::string>> representation;

			for (auto & rep_amount : rep_amounts)
			{
				auto const & account (rep_amount.first);
				auto const & amount (rep_amount.second);
				representation.emplace_back (amount, account.to_account ());
			}
			std::sort (representation.begin (), representation.end ());
			std::reverse (representation.begin (), representation.end ());
			for (auto i (representation.begin ()), n (representation.end ()); i != n && representatives.size () < count; ++i)
			{
				representatives.put (i->second, (i->first).convert_to<std::string> ());
			}
		}
		response_l.add_child ("representatives", representatives);
	}
	response_errors ();
}

void nano::json_handler::representatives_online ()
{
	auto const accounts_node = request.get_child_optional ("accounts");
	bool const weight = request.get<bool> ("weight", false);
	std::vector<nano::public_key> accounts_to_filter;
	if (accounts_node.is_initialized ())
	{
		for (auto & a : (*accounts_node))
		{
			auto account (account_impl (a.second.get<std::string> ("")));
			if (!ec)
			{
				accounts_to_filter.push_back (account);
			}
			else
			{
				break;
			}
		}
	}
	if (!ec)
	{
		boost::property_tree::ptree representatives;
		auto reps (node.online_reps.list ());
		for (auto & i : reps)
		{
			if (accounts_node.is_initialized ())
			{
				if (accounts_to_filter.empty ())
				{
					break;
				}
				auto found_acc = std::find (accounts_to_filter.begin (), accounts_to_filter.end (), i);
				if (found_acc == accounts_to_filter.end ())
				{
					continue;
				}
				else
				{
					accounts_to_filter.erase (found_acc);
				}
			}
			if (weight)
			{
				boost::property_tree::ptree weight_node;
				auto account_weight (node.ledger.weight (i));
				weight_node.put ("weight", account_weight.convert_to<std::string> ());
				representatives.add_child (i.to_account (), weight_node);
			}
			else
			{
				boost::property_tree::ptree entry;
				entry.put ("", i.to_account ());
				representatives.push_back (std::make_pair ("", entry));
			}
		}
		response_l.add_child ("representatives", representatives);
	}
	response_errors ();
}

void nano::json_handler::republish ()
{
	auto count (count_optional_impl (1024U));
	uint64_t sources (0);
	uint64_t destinations (0);
	boost::optional<std::string> sources_text (request.get_optional<std::string> ("sources"));
	if (!ec && sources_text.is_initialized ())
	{
		if (decode_unsigned (sources_text.get (), sources))
		{
			ec = nano::error_rpc::invalid_sources;
		}
	}
	boost::optional<std::string> destinations_text (request.get_optional<std::string> ("destinations"));
	if (!ec && destinations_text.is_initialized ())
	{
		if (decode_unsigned (destinations_text.get (), destinations))
		{
			ec = nano::error_rpc::invalid_destinations;
		}
	}
	auto hash (hash_impl ());
	if (!ec)
	{
		boost::property_tree::ptree blocks;
		auto transaction (node.store.tx_begin_read ());
		auto block (node.ledger.any ().block_get (*transaction, hash));
		if (block != nullptr)
		{
			std::deque<std::shared_ptr<nano::block>> republish_bundle;
			for (auto i (0); !hash.is_zero () && i < count; ++i)
			{
				block = node.ledger.any ().block_get (*transaction, hash);
				if (sources != 0) // Republish source chain
				{
					nano::block_hash source = block->source_field ().value_or (block->link_field ().value_or (0).as_block_hash ());
					auto block_a (node.ledger.any ().block_get (*transaction, source));
					std::vector<nano::block_hash> hashes;
					while (block_a != nullptr && hashes.size () < sources)
					{
						hashes.push_back (source);
						source = block_a->previous ();
						block_a = node.ledger.any ().block_get (*transaction, source);
					}
					std::reverse (hashes.begin (), hashes.end ());
					for (auto & hash_l : hashes)
					{
						block_a = node.ledger.any ().block_get (*transaction, hash_l);
						republish_bundle.push_back (std::move (block_a));
						boost::property_tree::ptree entry_l;
						entry_l.put ("", hash_l.to_string ());
						blocks.push_back (std::make_pair ("", entry_l));
					}
				}
				republish_bundle.push_back (std::move (block)); // Republish block
				boost::property_tree::ptree entry;
				entry.put ("", hash.to_string ());
				blocks.push_back (std::make_pair ("", entry));
				if (destinations != 0) // Republish destination chain
				{
					auto block_b (node.ledger.any ().block_get (*transaction, hash));
					auto destination = block_b->destination ();
					if (!destination.is_zero ())
					{
						if (!node.ledger.any ().pending_get (*transaction, nano::pending_key{ destination, hash }))
						{
							nano::block_hash previous (node.ledger.any ().account_head (*transaction, destination));
							auto block_d (node.ledger.any ().block_get (*transaction, previous));
							nano::block_hash source;
							std::vector<nano::block_hash> hashes;
							while (block_d != nullptr && hash != source)
							{
								hashes.push_back (previous);
								source = block_d->source_field ().value_or (block_d->is_send () ? 0 : block_d->link_field ().value_or (0).as_block_hash ());
								previous = block_d->previous ();
								block_d = node.ledger.any ().block_get (*transaction, previous);
							}
							std::reverse (hashes.begin (), hashes.end ());
							if (hashes.size () > destinations)
							{
								hashes.resize (destinations);
							}
							for (auto & hash_l : hashes)
							{
								block_d = node.ledger.any ().block_get (*transaction, hash_l);
								republish_bundle.push_back (std::move (block_d));
								boost::property_tree::ptree entry_l;
								entry_l.put ("", hash_l.to_string ());
								blocks.push_back (std::make_pair ("", entry_l));
							}
						}
					}
				}
				hash = node.ledger.any ().block_successor (*transaction, hash).value_or (0);
			}
			node.network->flood_block_many (std::move (republish_bundle), nullptr, 25);
			response_l.put ("success", ""); // obsolete
			response_l.add_child ("blocks", blocks);
		}
		else
		{
			ec = nano::error_blocks::not_found;
		}
	}
	response_errors ();
}

void nano::json_handler::search_pending ()
{
	response_l.put ("deprecated", "1");
	search_receivable ();
}

void nano::json_handler::search_receivable ()
{
	auto wallet_id (get_wallet_id ());
	if (!ec)
	{
		auto error = node.wallets.search_receivable (wallet_id);
		if (error == wallets_error::none)
		{
			response_l.put ("started", true);
		}
		else if (error == wallets_error::wallet_locked)
		{
			response_l.put ("started", false);
		}
		else
		{
			set_error (error);
		}
	}
	response_errors ();
}

void nano::json_handler::search_pending_all ()
{
	response_l.put ("deprecated", "1");
	search_receivable_all ();
}

void nano::json_handler::search_receivable_all ()
{
	if (!ec)
	{
		node.wallets.search_receivable_all ();
		response_l.put ("success", "");
	}
	response_errors ();
}

void nano::json_handler::send ()
{
	auto wallet_id (get_wallet_id ());
	auto amount (amount_impl ());
	// Sending 0 amount is invalid with state blocks
	if (!ec && amount.is_zero ())
	{
		ec = nano::error_common::invalid_amount;
	}
	std::string source_text (request.get<std::string> ("source"));
	auto source (account_impl (source_text, nano::error_rpc::bad_source));
	std::string destination_text (request.get<std::string> ("destination"));
	auto destination (account_impl (destination_text, nano::error_rpc::bad_destination));
	if (!ec)
	{
		auto work (work_optional_impl ());
		nano::uint128_t balance (0);
		if (!ec && work == 0 && !node.work_generation_enabled ())
		{
			ec = nano::error_common::disabled_work_generation;
		}
		if (!ec)
		{
			auto block_transaction (node.store.tx_begin_read ());
			auto info (account_info_impl (*block_transaction, source));
			if (!ec)
			{
				balance = (info.balance ()).number ();
			}
			if (!ec && work)
			{
				nano::block_details details (info.epoch (), true, false, false);
				if (node.network_params.work.difficulty (nano::work_version::work_1, info.head (), work) < node.network_params.work.threshold (nano::work_version::work_1, details))
				{
					ec = nano::error_common::invalid_work;
				}
			}
		}
		if (!ec)
		{
			bool generate_work (work == 0); // Disable work generation if "work" option is provided
			boost::optional<std::string> send_id (request.get_optional<std::string> ("id"));
			auto response_a (response);
			auto response_data (std::make_shared<boost::property_tree::ptree> (response_l));
			auto error = node.wallets.send_async (
			wallet_id, source, destination, amount.number (), [balance, amount, response_a, response_data] (std::shared_ptr<nano::block> const & block_a) {
				if (block_a != nullptr)
				{
					response_data->put ("block", block_a->hash ().to_string ());
					std::stringstream ostream;
					boost::property_tree::write_json (ostream, *response_data);
					response_a (ostream.str ());
				}
				else
				{
					if (balance >= amount.number ())
					{
						json_error_response (response_a, "Error generating block");
					}
					else
					{
						std::error_code ec (nano::error_common::insufficient_balance);
						json_error_response (response_a, ec.message ());
					}
				}
			},
			work, generate_work, send_id);

			set_error (error);
		}
	}
	// Because of send_async
	if (ec)
	{
		response_errors ();
	}
}

void nano::json_handler::sign ()
{
	bool const json_block_l = request.get<bool> ("json_block", false);
	// Retrieving hash
	nano::block_hash hash (0);
	boost::optional<std::string> hash_text (request.get_optional<std::string> ("hash"));
	if (hash_text.is_initialized ())
	{
		hash = hash_impl ();
	}
	// Retrieving block
	std::shared_ptr<nano::block> block;
	if (!ec && request.count ("block"))
	{
		block = block_impl (true);
		if (block != nullptr)
		{
			hash = block->hash ();
		}
	}

	// Hash or block are not initialized
	if (!ec && hash.is_zero ())
	{
		ec = nano::error_blocks::invalid_block;
	}
	// Hash is initialized without config permission
	else if (!ec && !hash.is_zero () && block == nullptr && !node_rpc_config.enable_sign_hash)
	{
		ec = nano::error_rpc::sign_hash_disabled;
	}
	if (!ec)
	{
		nano::raw_key prv;
		prv.clear ();
		// Retrieving private key from request
		boost::optional<std::string> key_text (request.get_optional<std::string> ("key"));
		if (key_text.is_initialized ())
		{
			if (prv.decode_hex (key_text.get ()))
			{
				ec = nano::error_common::bad_private_key;
			}
		}
		else
		{
			// Retrieving private key from wallet
			boost::optional<std::string> account_text (request.get_optional<std::string> ("account"));
			boost::optional<std::string> wallet_text (request.get_optional<std::string> ("wallet"));
			if (wallet_text.is_initialized () && account_text.is_initialized ())
			{
				auto account (account_impl ());
				auto wallet_id (get_wallet_id ());
				if (!ec)
				{
					auto error = node.wallets.fetch (wallet_id, account, prv);
					set_error (error);
				}
			}
		}
		// Signing
		if (prv != 0)
		{
			nano::public_key pub (nano::pub_key (prv));
			nano::signature signature (nano::sign_message (prv, pub, hash));
			response_l.put ("signature", signature.to_string ());
			if (block != nullptr)
			{
				block->signature_set (signature);

				if (json_block_l)
				{
					boost::property_tree::ptree block_node_l;
					block->serialize_json (block_node_l);
					response_l.add_child ("block", block_node_l);
				}
				else
				{
					std::string contents;
					block->serialize_json (contents);
					response_l.put ("block", contents);
				}
			}
		}
		else
		{
			ec = nano::error_rpc::block_create_key_required;
		}
	}
	response_errors ();
}

void nano::json_handler::stats ()
{
	auto sink = node.stats->log_sink_json ();
	std::string type (request.get<std::string> ("type", ""));
	bool use_sink = false;
	if (type == "counters")
	{
		node.stats->log_counters (*sink);
		use_sink = true;
	}
	else if (type == "objects")
	{
		construct_json (collect_container_info (node, "node").get (), response_l);
	}
	else if (type == "samples")
	{
		node.stats->log_samples (*sink);
		use_sink = true;
	}
	else if (type == "database")
	{
		node.store.serialize_memory_stats (response_l);
	}
	else
	{
		ec = nano::error_rpc::invalid_missing_type;
	}
	if (!ec && use_sink)
	{
		auto stat_tree_l (*static_cast<boost::property_tree::ptree *> (sink->to_object ()));
		stat_tree_l.put ("stat_duration_seconds", node.stats->last_reset ().count ());
		std::stringstream ostream;
		boost::property_tree::write_json (ostream, stat_tree_l);
		response (ostream.str ());
	}
	else
	{
		response_errors ();
	}
}

void nano::json_handler::stats_clear ()
{
	node.stats->clear ();
	response_l.put ("success", "");
	std::stringstream ostream;
	boost::property_tree::write_json (ostream, response_l);
	response (ostream.str ());
}

void nano::json_handler::stop ()
{
	response_l.put ("success", "");
	response_errors ();
	if (!ec)
	{
		stop_callback ();
	}
}

void nano::json_handler::telemetry ()
{
	auto address_text (request.get_optional<std::string> ("address"));
	auto port_text (request.get_optional<std::string> ("port"));

	if (address_text.is_initialized () || port_text.is_initialized ())
	{
		// Check both are specified
		nano::endpoint endpoint{};
		if (address_text.is_initialized () && port_text.is_initialized ())
		{
			uint16_t port;
			if (!nano::parse_port (*port_text, port))
			{
				boost::asio::ip::address address;
				if (!nano::parse_address (*address_text, address))
				{
					endpoint = { address, port };

					if (address.is_loopback () && port == node.network->endpoint ().port ())
					{
						// Requesting telemetry metrics locally
						auto telemetry_data = node.local_telemetry ();

						nano::jsonconfig config_l;
						auto const should_ignore_identification_metrics = false;
						auto err = telemetry_data.serialize_json (config_l, should_ignore_identification_metrics);
						auto const & ptree = config_l.get_tree ();

						if (!err)
						{
							response_l.insert (response_l.begin (), ptree.begin (), ptree.end ());
						}

						response_errors ();
						return;
					}
				}
				else
				{
					ec = nano::error_common::invalid_ip_address;
				}
			}
			else
			{
				ec = nano::error_common::invalid_port;
			}
		}
		else
		{
			ec = nano::error_rpc::requires_port_and_address;
		}

		if (!ec)
		{
			auto maybe_telemetry = node.telemetry->get_telemetry (nano::transport::map_endpoint_to_v6 (endpoint));
			if (maybe_telemetry)
			{
				auto telemetry = *maybe_telemetry;
				nano::jsonconfig config_l;
				auto const should_ignore_identification_metrics = false;
				auto err = telemetry.serialize_json (config_l, should_ignore_identification_metrics);
				auto const & ptree = config_l.get_tree ();

				if (!err)
				{
					response_l.insert (response_l.begin (), ptree.begin (), ptree.end ());
				}
				else
				{
					ec = nano::error_rpc::generic;
				}
			}
			else
			{
				ec = nano::error_rpc::peer_not_found;
			}

			response_errors ();
		}
		else
		{
			response_errors ();
		}
	}
	else
	{
		// By default, consolidated (average or mode) telemetry metrics are returned,
		// setting "raw" to true returns metrics from all nodes requested.
		auto raw = request.get_optional<bool> ("raw");
		auto output_raw = raw.value_or (false);

		auto telemetry_responses = node.telemetry->get_all_telemetries ();
		if (output_raw)
		{
			boost::property_tree::ptree metrics;
			for (auto & telemetry_metrics : telemetry_responses)
			{
				nano::jsonconfig config_l;
				auto const should_ignore_identification_metrics = false;
				auto err = telemetry_metrics.second.serialize_json (config_l, should_ignore_identification_metrics);
				config_l.put ("address", telemetry_metrics.first.address ());
				config_l.put ("port", telemetry_metrics.first.port ());
				if (!err)
				{
					metrics.push_back (std::make_pair ("", config_l.get_tree ()));
				}
				else
				{
					ec = nano::error_rpc::generic;
				}
			}

			response_l.put_child ("metrics", metrics);
		}
		else
		{
			// Default case without any parameters, requesting telemetry metrics locally
			auto telemetry_data = node.local_telemetry ();

			nano::jsonconfig config_l;
			auto const should_ignore_identification_metrics = false;
			auto err = telemetry_data.serialize_json (config_l, should_ignore_identification_metrics);
			auto const & ptree = config_l.get_tree ();

			if (!err)
			{
				response_l.insert (response_l.begin (), ptree.begin (), ptree.end ());
			}

			response_errors ();
			return;
		}

		response_errors ();
	}
}

void nano::json_handler::unchecked ()
{
	bool const json_block_l = request.get<bool> ("json_block", false);
	auto count (count_optional_impl ());
	if (!ec)
	{
		boost::property_tree::ptree unchecked;
		node.unchecked.for_each (
		[&unchecked, &json_block_l] (nano::unchecked_key const & key, nano::unchecked_info const & info) {
			auto block = info.get_block ();
			if (json_block_l)
			{
				boost::property_tree::ptree block_node_l;
				block->serialize_json (block_node_l);
				unchecked.add_child (block->hash ().to_string (), block_node_l);
			}
			else
			{
				std::string contents;
				block->serialize_json (contents);
				unchecked.put (block->hash ().to_string (), contents);
			} }, [iterations = 0, count = count] () mutable { return iterations++ < count; });
		response_l.add_child ("blocks", unchecked);
	}
	response_errors ();
}

void nano::json_handler::unchecked_clear ()
{
	node.workers->push_task (create_worker_task ([] (std::shared_ptr<nano::json_handler> const & rpc_l) {
		rpc_l->node.unchecked.clear ();
		rpc_l->response_l.put ("success", "");
		rpc_l->response_errors ();
	}));
}

void nano::json_handler::unchecked_get ()
{
	bool const json_block_l = request.get<bool> ("json_block", false);
	auto hash (hash_impl ());
	if (!ec)
	{
		bool done = false;
		node.unchecked.for_each (
		[&] (nano::unchecked_key const & key, nano::unchecked_info const & info) {
			if (key.hash == hash)
			{
				response_l.put ("modified_timestamp", std::to_string (info.modified ()));

				auto block = info.get_block ();
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
				done = true;
			} }, [&] () { return !done; });
		if (response_l.empty ())
		{
			ec = nano::error_blocks::not_found;
		}
	}
	response_errors ();
}

void nano::json_handler::unchecked_keys ()
{
	bool const json_block_l = request.get<bool> ("json_block", false);
	auto count (count_optional_impl ());
	nano::block_hash key (0);
	boost::optional<std::string> hash_text (request.get_optional<std::string> ("key"));
	if (!ec && hash_text.is_initialized ())
	{
		if (key.decode_hex (hash_text.get ()))
		{
			ec = nano::error_rpc::bad_key;
		}
	}
	if (!ec)
	{
		boost::property_tree::ptree unchecked;
		node.unchecked.for_each (
		key,
		[&unchecked, json_block_l] (nano::unchecked_key const & key, nano::unchecked_info const & info) {
			boost::property_tree::ptree entry;
			auto block = info.get_block ();
			entry.put ("key", key.key ().to_string ());
			entry.put ("hash", block->hash ().to_string ());
			entry.put ("modified_timestamp", std::to_string (info.modified ()));
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
			unchecked.push_back (std::make_pair ("", entry)); }, [&unchecked, &count] () { return unchecked.size () < count; });
		response_l.add_child ("unchecked", unchecked);
	}
	response_errors ();
}

void nano::json_handler::unopened ()
{
	auto count (count_optional_impl ());
	auto threshold (threshold_optional_impl ());
	nano::account start{ 1 }; // exclude burn account by default
	boost::optional<std::string> account_text (request.get_optional<std::string> ("account"));
	if (account_text.is_initialized ())
	{
		start = account_impl (account_text.get ());
	}
	if (!ec)
	{
		auto transaction = node.store.tx_begin_read ();
		auto & ledger = node.ledger;
		boost::property_tree::ptree accounts;
		for (auto iterator = ledger.any ().receivable_upper_bound (*transaction, start, 0); !iterator.is_end () && accounts.size () < count;)
		{
			auto const & [key, info] = *iterator;
			nano::account account = key.account;
			if (!node.store.account ().exists (*transaction, account))
			{
				nano::uint128_t current_account_sum{ 0 };
				while (!iterator.is_end ())
				{
					auto const & [key, info] = *iterator;
					current_account_sum += info.amount.number ();
					++iterator;
				}
				if (current_account_sum >= threshold.number ())
				{
					accounts.put (account.to_account (), current_account_sum.convert_to<std::string> ());
				}
			}
			iterator = ledger.any ().receivable_upper_bound (*transaction, account);
		}
		response_l.add_child ("accounts", accounts);
	}
	response_errors ();
}

void nano::json_handler::uptime ()
{
	response_l.put ("seconds", std::chrono::duration_cast<std::chrono::seconds> (std::chrono::steady_clock::now () - node.startup_time).count ());
	response_errors ();
}

void nano::json_handler::version ()
{
	response_l.put ("rpc_version", "1");
	response_l.put ("store_version", std::to_string (node.store_version ()));
	response_l.put ("protocol_version", std::to_string (node.network_params.network.protocol_version));
	response_l.put ("node_vendor", boost::str (boost::format ("RsNano %1%") % NANO_VERSION_STRING));
	response_l.put ("store_vendor", node.store.vendor_get ());
	response_l.put ("network", node.network_params.network.get_current_network_as_string ());
	response_l.put ("network_identifier", node.network_params.ledger.genesis->hash ().to_string ());
	response_l.put ("build_info", BUILD_INFO);
	response_errors ();
}

void nano::json_handler::validate_account_number ()
{
	auto account (account_impl ());
	(void)account;
	response_l.put ("valid", ec ? "0" : "1");
	ec = std::error_code (); // error is just invalid account
	response_errors ();
}

void nano::json_handler::wallet_add ()
{
	node.workers->push_task (create_worker_task ([] (std::shared_ptr<nano::json_handler> const & rpc_l) {
		auto wallet_id (rpc_l->get_wallet_id ());
		if (!rpc_l->ec)
		{
			std::string key_text (rpc_l->request.get<std::string> ("key"));
			nano::raw_key key;
			if (!key.decode_hex (key_text))
			{
				bool const generate_work = rpc_l->request.get<bool> ("work", true);
				nano::public_key pub;
				auto error = rpc_l->node.wallets.insert_adhoc (wallet_id, key, generate_work, pub);
				if (error == nano::wallets_error::none)
				{
					rpc_l->response_l.put ("account", pub.to_account ());
				}
				else
				{
					rpc_l->set_error (error);
				}
			}
			else
			{
				rpc_l->ec = nano::error_common::bad_private_key;
			}
		}
		rpc_l->response_errors ();
	}));
}

void nano::json_handler::wallet_add_watch ()
{
	node.workers->push_task (create_worker_task ([] (std::shared_ptr<nano::json_handler> const & rpc_l) {
		auto wallet_id (rpc_l->get_wallet_id ());
		if (!rpc_l->ec)
		{
			std::vector<nano::account> accounts;
			for (auto & accs : rpc_l->request.get_child ("accounts"))
			{
				auto account (rpc_l->account_impl (accs.second.data ()));
				if (!rpc_l->ec)
				{
					accounts.push_back (account);
				}
			}

			if (!rpc_l->ec)
			{
				auto error = rpc_l->node.wallets.insert_watch (wallet_id, accounts);
				if (error == nano::wallets_error::none)
				{
					rpc_l->response_l.put ("success", "");
				}
				else
				{
					rpc_l->set_error (error);
				}
			}
		}
		rpc_l->response_errors ();
	}));
}

void nano::json_handler::wallet_info ()
{
	auto wallet_id (get_wallet_id ());
	if (!ec)
	{
		std::vector<std::pair<nano::account, nano::raw_key>> accounts;
		auto error = node.wallets.decrypt (wallet_id, accounts);
		set_error (error);
		if (error == nano::wallets_error::none)
		{
			nano::uint128_t balance (0);
			nano::uint128_t receivable (0);
			uint64_t count (0);
			uint64_t block_count (0);
			uint64_t cemented_block_count (0);
			uint64_t deterministic_count (0);
			uint64_t adhoc_count (0);
			auto block_transaction (node.store.tx_begin_read ());

			for (const auto & [account, priv] : accounts)
			{
				auto account_info = node.ledger.any ().account_get (*block_transaction, account);
				if (account_info)
				{
					block_count += account_info->block_count ();
					balance += account_info->balance ().number ();
				}

				nano::confirmation_height_info confirmation_info{};
				if (!node.store.confirmation_height ().get (*block_transaction, account, confirmation_info))
				{
					cemented_block_count += confirmation_info.height ();
				}

				receivable += node.ledger.account_receivable (*block_transaction, account);

				nano::key_type key_type = node.wallets.key_type (wallet_id, account);
				if (key_type == nano::key_type::deterministic)
				{
					deterministic_count++;
				}
				else if (key_type == nano::key_type::adhoc)
				{
					adhoc_count++;
				}

				++count;
			}

			uint32_t deterministic_index;
			(void)node.wallets.deterministic_index_get (wallet_id, deterministic_index);
			response_l.put ("balance", balance.convert_to<std::string> ());
			response_l.put ("pending", receivable.convert_to<std::string> ());
			response_l.put ("receivable", receivable.convert_to<std::string> ());
			response_l.put ("accounts_count", std::to_string (count));
			response_l.put ("accounts_block_count", std::to_string (block_count));
			response_l.put ("accounts_cemented_block_count", std::to_string (cemented_block_count));
			response_l.put ("deterministic_count", std::to_string (deterministic_count));
			response_l.put ("adhoc_count", std::to_string (adhoc_count));
			response_l.put ("deterministic_index", std::to_string (deterministic_index));
		}
	}

	response_errors ();
}

void nano::json_handler::wallet_balances ()
{
	auto wallet_id (get_wallet_id ());
	auto threshold (threshold_optional_impl ());
	if (!ec)
	{
		boost::property_tree::ptree balances;
		auto block_transaction (node.store.tx_begin_read ());
		std::vector<nano::account> accounts;
		auto error = node.wallets.get_accounts (wallet_id, accounts);
		if (error == nano::wallets_error::none)
		{
			for (const auto & account : accounts)
			{
				nano::uint128_t balance = node.ledger.any ().account_balance (*block_transaction, account).value_or (0).number ();
				if (balance >= threshold.number ())
				{
					boost::property_tree::ptree entry;
					nano::uint128_t receivable = node.ledger.account_receivable (*block_transaction, account);
					entry.put ("balance", balance.convert_to<std::string> ());
					entry.put ("pending", receivable.convert_to<std::string> ());
					entry.put ("receivable", receivable.convert_to<std::string> ());
					balances.push_back (std::make_pair (account.to_account (), entry));
				}
			}
			response_l.add_child ("balances", balances);
		}
		else
		{
			set_error (error);
		}
	}
	response_errors ();
}

void nano::json_handler::wallet_change_seed ()
{
	node.workers->push_task (create_worker_task ([] (std::shared_ptr<nano::json_handler> const & rpc_l) {
		auto wallet_id (rpc_l->get_wallet_id ());
		if (!rpc_l->ec)
		{
			std::string seed_text (rpc_l->request.get<std::string> ("seed"));
			nano::raw_key seed;
			if (!seed.decode_hex (seed_text))
			{
				auto count (static_cast<uint32_t> (rpc_l->count_optional_impl (0)));

				uint32_t restored_count = 0;
				nano::account first_account;

				auto error = rpc_l->node.wallets.change_seed (wallet_id, seed, count, first_account, restored_count);
				if (error == nano::wallets_error::none)
				{
					rpc_l->response_l.put ("success", "");
					rpc_l->response_l.put ("last_restored_account", first_account.to_account ());
					debug_assert (restored_count > 0);
					rpc_l->response_l.put ("restored_count", std::to_string (restored_count));
				}
				else
				{
					rpc_l->set_error (error);
				}
			}
			else
			{
				rpc_l->ec = nano::error_common::bad_seed;
			}
		}
		rpc_l->response_errors ();
	}));
}

void nano::json_handler::wallet_contains ()
{
	auto account (account_impl ());
	auto wallet_id (get_wallet_id ());
	if (!ec)
	{
		std::vector<nano::account> accounts;
		auto error = node.wallets.get_accounts (wallet_id, accounts);
		if (error == nano::wallets_error::none)
		{
			bool exists = std::find (accounts.begin (), accounts.end (), account) != accounts.end ();
			response_l.put ("exists", exists ? "1" : "0");
		}
	}
	response_errors ();
}

void nano::json_handler::wallet_create ()
{
	node.workers->push_task (create_worker_task ([] (std::shared_ptr<nano::json_handler> const & rpc_l) {
		nano::raw_key seed;
		auto seed_text (rpc_l->request.get_optional<std::string> ("seed"));
		if (seed_text.is_initialized () && seed.decode_hex (seed_text.get ()))
		{
			rpc_l->ec = nano::error_common::bad_seed;
		}
		if (!rpc_l->ec)
		{
			auto wallet_id = random_wallet_id ();
			rpc_l->node.wallets.create (wallet_id);
			if (rpc_l->node.wallets.wallet_exists (wallet_id))
			{
				rpc_l->response_l.put ("wallet", wallet_id.to_string ());
			}
			else
			{
				rpc_l->ec = nano::error_common::wallet_lmdb_max_dbs;
			}
			if (!rpc_l->ec && seed_text.is_initialized ())
			{
				nano::account first_account;
				uint32_t restored_count;
				auto error = rpc_l->node.wallets.change_seed (wallet_id, seed, 0, first_account, restored_count);
				rpc_l->response_l.put ("last_restored_account", first_account.to_account ());
				debug_assert (restored_count > 0);
				rpc_l->response_l.put ("restored_count", std::to_string (restored_count));
			}
		}
		rpc_l->response_errors ();
	}));
}

void nano::json_handler::wallet_destroy ()
{
	node.workers->push_task (create_worker_task ([] (std::shared_ptr<nano::json_handler> const & rpc_l) {
		std::string wallet_text (rpc_l->request.get<std::string> ("wallet"));
		nano::wallet_id wallet;
		if (!wallet.decode_hex (wallet_text))
		{
			if (rpc_l->node.wallets.wallet_exists (wallet))
			{
				rpc_l->node.wallets.destroy (wallet);
				bool destroyed (!rpc_l->node.wallets.wallet_exists (wallet));
				rpc_l->response_l.put ("destroyed", destroyed ? "1" : "0");
			}
			else
			{
				rpc_l->ec = nano::error_common::wallet_not_found;
			}
		}
		else
		{
			rpc_l->ec = nano::error_common::bad_wallet_number;
		}
		rpc_l->response_errors ();
	}));
}

void nano::json_handler::wallet_export ()
{
	auto wallet_id (get_wallet_id ());
	if (!ec)
	{
		std::string json;
		auto error = node.wallets.serialize (wallet_id, json);
		if (error == nano::wallets_error::none)
		{
			response_l.put ("json", json);
		}
		else
		{
			set_error (error);
		}
	}
	response_errors ();
}

void nano::json_handler::wallet_frontiers ()
{
	auto wallet_id (get_wallet_id ());
	if (!ec)
	{
		boost::property_tree::ptree frontiers;
		auto block_transaction (node.store.tx_begin_read ());
		std::vector<nano::account> accounts;
		auto error = node.wallets.get_accounts (wallet_id, accounts);
		if (error == nano::wallets_error::none)
		{
			for (const auto & account : accounts)
			{
				auto latest (node.ledger.any ().account_head (*block_transaction, account));
				if (!latest.is_zero ())
				{
					frontiers.put (account.to_account (), latest.to_string ());
				}
			}
			response_l.add_child ("frontiers", frontiers);
		}
		else
		{
			set_error (error);
		}
	}
	response_errors ();
}

void nano::json_handler::wallet_history ()
{
	uint64_t modified_since (1);
	boost::optional<std::string> modified_since_text (request.get_optional<std::string> ("modified_since"));
	if (modified_since_text.is_initialized ())
	{
		if (decode_unsigned (modified_since_text.get (), modified_since))
		{
			ec = nano::error_rpc::invalid_timestamp;
		}
	}
	auto wallet_id (get_wallet_id ());
	if (!ec)
	{
		std::vector<nano::account> accounts;
		auto error = node.wallets.get_accounts (wallet_id, accounts);
		if (error == nano::wallets_error::none)
		{
			std::multimap<uint64_t, boost::property_tree::ptree, std::greater<uint64_t>> entries;
			auto block_transaction (node.store.tx_begin_read ());
			for (const auto & account : accounts)
			{
				auto info = node.ledger.any ().account_get (*block_transaction, account);
				if (info)
				{
					auto timestamp (info->modified ());
					auto hash (info->head ());
					while (timestamp >= modified_since && !hash.is_zero ())
					{
						auto block (node.ledger.any ().block_get (*block_transaction, hash));
						timestamp = block->sideband ().timestamp ();
						if (block != nullptr && timestamp >= modified_since)
						{
							boost::property_tree::ptree entry;
							std::vector<nano::public_key> no_filter;
							history_visitor visitor (*this, false, *block_transaction, entry, hash, no_filter);
							block->visit (visitor);
							if (!entry.empty ())
							{
								entry.put ("block_account", account.to_account ());
								entry.put ("hash", hash.to_string ());
								entry.put ("local_timestamp", std::to_string (timestamp));
								entries.insert (std::make_pair (timestamp, entry));
							}
							hash = block->previous ();
						}
						else
						{
							hash.clear ();
						}
					}
				}
			}
			boost::property_tree::ptree history;
			for (auto i (entries.begin ()), n (entries.end ()); i != n; ++i)
			{
				history.push_back (std::make_pair ("", i->second));
			}
			response_l.add_child ("history", history);
		}
		else
		{
			set_error (error);
		}
	}
	response_errors ();
}

void nano::json_handler::wallet_key_valid ()
{
	auto wallet_id (get_wallet_id ());
	if (!ec)
	{
		bool valid = false;
		auto error = node.wallets.valid_password (wallet_id, valid);
		if (error == nano::wallets_error::none)
		{
			response_l.put ("valid", valid ? "1" : "0");
		}
		else
		{
			set_error (error);
		}
	}
	response_errors ();
}

void nano::json_handler::wallet_ledger ()
{
	bool const representative = request.get<bool> ("representative", false);
	bool const weight = request.get<bool> ("weight", false);
	bool const pending = request.get<bool> ("pending", false);
	bool const receivable = request.get<bool> ("receivable", pending);
	uint64_t modified_since (0);
	boost::optional<std::string> modified_since_text (request.get_optional<std::string> ("modified_since"));
	if (modified_since_text.is_initialized ())
	{
		modified_since = strtoul (modified_since_text.get ().c_str (), NULL, 10);
	}
	auto wallet_id (get_wallet_id ());
	if (!ec)
	{
		std::vector<nano::account> accounts;
		auto error = node.wallets.get_accounts (wallet_id, accounts);
		if (error == nano::wallets_error::none)
		{
			boost::property_tree::ptree accounts_json;
			auto block_transaction (node.store.tx_begin_read ());
			for (const auto & account : accounts)
			{
				auto info = node.ledger.any ().account_get (*block_transaction, account);
				if (info)
				{
					if (info->modified () >= modified_since)
					{
						boost::property_tree::ptree entry;
						entry.put ("frontier", info->head ().to_string ());
						entry.put ("open_block", info->open_block ().to_string ());
						entry.put ("representative_block", node.ledger.representative (*block_transaction, info->head ()).to_string ());
						std::string balance;
						nano::uint128_union (info->balance ()).encode_dec (balance);
						entry.put ("balance", balance);
						entry.put ("modified_timestamp", std::to_string (info->modified ()));
						entry.put ("block_count", std::to_string (info->block_count ()));
						if (representative)
						{
							entry.put ("representative", info->representative ().to_account ());
						}
						if (weight)
						{
							auto account_weight (node.ledger.weight_exact (*block_transaction, account));
							entry.put ("weight", account_weight.convert_to<std::string> ());
						}
						if (receivable)
						{
							auto account_receivable (node.ledger.account_receivable (*block_transaction, account));
							entry.put ("pending", account_receivable.convert_to<std::string> ());
							entry.put ("receivable", account_receivable.convert_to<std::string> ());
						}
						accounts_json.push_back (std::make_pair (account.to_account (), entry));
					}
				}
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

void nano::json_handler::wallet_lock ()
{
	auto wallet_id (get_wallet_id ());
	if (!ec)
	{
		auto error = node.wallets.lock (wallet_id);
		if (error == nano::wallets_error::none)
		{
			response_l.put ("locked", "1");

			node.logger->warn (nano::log::type::rpc, "Wallet locked");
		}
		else
		{
			set_error (error);
		}
	}
	response_errors ();
}

void nano::json_handler::wallet_pending ()
{
	response_l.put ("deprecated", "1");
	wallet_receivable ();
}

void nano::json_handler::wallet_receivable ()
{
	auto wallet_id (get_wallet_id ());
	auto count (count_optional_impl ());
	auto threshold (threshold_optional_impl ());
	bool const source = request.get<bool> ("source", false);
	bool const min_version = request.get<bool> ("min_version", false);
	bool const include_active = request.get<bool> ("include_active", false);
	bool const include_only_confirmed = request.get<bool> ("include_only_confirmed", true);
	if (!ec)
	{
		std::vector<nano::account> accounts;
		auto error = node.wallets.get_accounts (wallet_id, accounts);
		if (error == nano::wallets_error::none)
		{
			boost::property_tree::ptree pending;
			auto block_transaction = node.store.tx_begin_read ();
			for (const auto & account : accounts)
			{
				boost::property_tree::ptree peers_l;
				for (auto current = node.ledger.any ().receivable_upper_bound (*block_transaction, account, 0); !current.is_end () && peers_l.size () < count; ++current)
				{
					auto const & [key, info] = *current;
					if (include_only_confirmed && !node.ledger.confirmed ().block_exists_or_pruned (*block_transaction, key.hash))
					{
						continue;
					}
					if (threshold.is_zero () && !source)
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
					if (source || min_version)
					{
						boost::property_tree::ptree pending_tree;
						pending_tree.put ("amount", info.amount.number ().template convert_to<std::string> ());
						if (source)
						{
							pending_tree.put ("source", info.source.to_account ());
						}
						if (min_version)
						{
							pending_tree.put ("min_version", epoch_as_string (info.epoch));
						}
						peers_l.add_child (key.hash.to_string (), pending_tree);
					}
					else
					{
						peers_l.put (key.hash.to_string (), info.amount.number ().template convert_to<std::string> ());
					}
				}
				if (!peers_l.empty ())
				{
					pending.add_child (account.to_account (), peers_l);
				}
			}
			response_l.add_child ("blocks", pending);
		}
		else
		{
			set_error (error);
		}
	}
	response_errors ();
}

void nano::json_handler::wallet_representative ()
{
	auto wallet_id (get_wallet_id ());
	if (!ec)
	{
		nano::account representative;
		auto error = node.wallets.get_representative (wallet_id, representative);
		if (error == nano::wallets_error::none)
		{
			response_l.put ("representative", representative.to_account ());
		}
		else
		{
			set_error (error);
		}
	}
	response_errors ();
}

void nano::json_handler::wallet_representative_set ()
{
	node.workers->push_task (create_worker_task ([] (std::shared_ptr<nano::json_handler> const & rpc_l) {
		auto wallet_id (rpc_l->get_wallet_id ());
		std::string representative_text (rpc_l->request.get<std::string> ("representative"));
		auto representative (rpc_l->account_impl (representative_text, nano::error_rpc::bad_representative_number));
		if (!rpc_l->ec)
		{
			bool update_existing_accounts (rpc_l->request.get<bool> ("update_existing_accounts", false));
			auto error = rpc_l->node.wallets.set_representative (wallet_id, representative, update_existing_accounts);
			if (error == nano::wallets_error::none)
			{
				rpc_l->response_l.put ("set", "1");
			}
			else
			{
				rpc_l->set_error (error);
			}
		}
		rpc_l->response_errors ();
	}));
}

void nano::json_handler::wallet_republish ()
{
	auto wallet_id (get_wallet_id ());
	auto count (count_impl ());
	if (!ec)
	{
		std::vector<nano::account> accounts;
		auto error = node.wallets.get_accounts (wallet_id, accounts);
		if (error == nano::wallets_error::none)
		{
			boost::property_tree::ptree blocks;
			std::deque<std::shared_ptr<nano::block>> republish_bundle;
			auto block_transaction (node.store.tx_begin_read ());
			for (const auto & account : accounts)
			{
				auto latest (node.ledger.any ().account_head (*block_transaction, account));
				std::shared_ptr<nano::block> block;
				std::vector<nano::block_hash> hashes;
				while (!latest.is_zero () && hashes.size () < count)
				{
					hashes.push_back (latest);
					block = node.ledger.any ().block_get (*block_transaction, latest);
					if (block != nullptr)
					{
						latest = block->previous ();
					}
					else
					{
						latest.clear ();
					}
				}
				std::reverse (hashes.begin (), hashes.end ());
				for (auto & hash : hashes)
				{
					block = node.ledger.any ().block_get (*block_transaction, hash);
					republish_bundle.push_back (std::move (block));
					boost::property_tree::ptree entry;
					entry.put ("", hash.to_string ());
					blocks.push_back (std::make_pair ("", entry));
				}
			}
			node.network->flood_block_many (std::move (republish_bundle), nullptr, 25);
			response_l.add_child ("blocks", blocks);
		}
		else
		{
			set_error (error);
		}
	}
	response_errors ();
}

void nano::json_handler::wallet_seed ()
{
	auto wallet_id (get_wallet_id ());
	if (!ec)
	{
		nano::raw_key seed;
		auto error = node.wallets.get_seed (wallet_id, seed);
		if (error == nano::wallets_error::none)
		{
			response_l.put ("seed", seed.to_string ());
		}
		else
		{
			set_error (error);
		}
	}
	response_errors ();
}

void nano::json_handler::wallet_work_get ()
{
	auto wallet_id (get_wallet_id ());
	if (!ec)
	{
		std::vector<nano::account> accounts;
		auto error = node.wallets.get_accounts (wallet_id, accounts);
		if (error == nano::wallets_error::none)
		{
			boost::property_tree::ptree works;
			for (const auto & account : accounts)
			{
				uint64_t work = node.wallets.work_get (wallet_id, account);
				works.put (account.to_account (), nano::to_string_hex (work));
			}
			response_l.add_child ("works", works);
		}
		else
		{
			set_error (error);
		}
	}
	response_errors ();
}

void nano::json_handler::work_generate ()
{
	std::optional<nano::account> account;
	auto account_opt (request.get_optional<std::string> ("account"));
	// Default to work_1 if not specified
	auto work_version (work_version_optional_impl (nano::work_version::work_1));
	if (!ec && account_opt.is_initialized ())
	{
		account = account_impl (account_opt.get ());
	}
	if (!ec)
	{
		auto hash (hash_impl ());
		auto difficulty (difficulty_optional_impl (work_version));
		multiplier_optional_impl (work_version, difficulty);
		if (!ec && (difficulty > node.max_work_generate_difficulty (work_version) || difficulty < node.network_params.work.threshold_entry (work_version, nano::block_type::state)))
		{
			ec = nano::error_rpc::difficulty_limit;
		}
		// Retrieving optional block
		std::shared_ptr<nano::block> block;
		if (!ec && request.count ("block"))
		{
			block = block_impl (true);
			if (block != nullptr)
			{
				if (hash != block->root ().as_block_hash ())
				{
					ec = nano::error_rpc::block_root_mismatch;
				}
				if (request.count ("version") == 0)
				{
					work_version = block->work_version ();
				}
				else if (!ec && work_version != block->work_version ())
				{
					ec = nano::error_rpc::block_work_version_mismatch;
				}
				// Difficulty calculation
				if (!ec && request.count ("difficulty") == 0 && request.count ("multiplier") == 0)
				{
					difficulty = difficulty_ledger (*block);
				}
				// If optional block difficulty is higher than requested difficulty, send error
				if (!ec && node.network_params.work.difficulty (*block) >= difficulty)
				{
					ec = nano::error_rpc::block_work_enough;
				}
			}
		}
		if (!ec && response_l.empty ())
		{
			auto use_peers (request.get<bool> ("use_peers", false));
			auto rpc_l (shared_from_this ());
			auto callback = [rpc_l, hash, work_version, this] (std::optional<uint64_t> const & work_a) {
				if (work_a)
				{
					boost::property_tree::ptree response_l;
					response_l.put ("hash", hash.to_string ());
					uint64_t work (work_a.value ());
					response_l.put ("work", nano::to_string_hex (work));
					std::stringstream ostream;
					auto result_difficulty (rpc_l->node.network_params.work.difficulty (work_version, hash, work));
					response_l.put ("difficulty", nano::to_string_hex (result_difficulty));
					auto result_multiplier = nano::difficulty::to_multiplier (result_difficulty, node.default_difficulty (work_version));
					response_l.put ("multiplier", nano::to_string (result_multiplier));
					boost::property_tree::write_json (ostream, response_l);
					rpc_l->response (ostream.str ());
				}
				else
				{
					json_error_response (rpc_l->response, "Cancelled");
				}
			};
			if (!use_peers)
			{
				if (node.local_work_generation_enabled ())
				{
					node.distributed_work.make (work_version, hash, {}, difficulty, callback, {});
				}
				else
				{
					ec = nano::error_common::disabled_local_work_generation;
				}
			}
			else
			{
				if (!account_opt.is_initialized ())
				{
					// Fetch account from block if not given
					auto transaction_l (node.store.tx_begin_read ());
					if (node.ledger.any ().block_exists (*transaction_l, hash))
					{
						account = node.ledger.any ().block_account (*transaction_l, hash).value ();
					}
				}
				auto secondary_work_peers_l (request.get<bool> ("secondary_work_peers", false));
				if (node.distributed_work.work_generation_enabled (secondary_work_peers_l))
				{
					node.work_generate (work_version, hash, difficulty, callback, account, secondary_work_peers_l);
				}
				else
				{
					ec = nano::error_common::disabled_work_generation;
				}
			}
		}
	}
	// Because of callback
	if (ec)
	{
		response_errors ();
	}
}

void nano::json_handler::work_cancel ()
{
	auto hash (hash_impl ());
	if (!ec)
	{
		node.distributed_work.cancel (hash);
		response_l.put ("success", "");
	}
	response_errors ();
}

void nano::json_handler::work_get ()
{
	auto wallet_id (get_wallet_id ());
	auto account (account_impl ());
	if (!ec)
	{
		uint64_t work (0);
		auto error = node.wallets.work_get (wallet_id, account, work);
		if (error == nano::wallets_error::none)
		{
			response_l.put ("work", nano::to_string_hex (work));
		}
		else
		{
			set_error (error);
		}
	}
	response_errors ();
}

void nano::json_handler::work_set ()
{
	node.workers->push_task (create_worker_task ([] (std::shared_ptr<nano::json_handler> const & rpc_l) {
		auto wallet_id (rpc_l->get_wallet_id ());
		auto account (rpc_l->account_impl ());
		auto work (rpc_l->work_optional_impl ());
		if (!rpc_l->ec)
		{
			auto error = rpc_l->node.wallets.work_set (wallet_id, account, work);
			if (error == nano::wallets_error::none)
			{
				rpc_l->response_l.put ("success", "");
			}
			else
			{
				rpc_l->set_error (error);
			}
		}
		rpc_l->response_errors ();
	}));
}

void nano::json_handler::work_validate ()
{
	auto hash (hash_impl ());
	auto work (work_optional_impl ());
	// Default to work_1 if not specified
	auto work_version (work_version_optional_impl (nano::work_version::work_1));
	auto difficulty (difficulty_optional_impl (work_version));
	multiplier_optional_impl (work_version, difficulty);
	if (!ec)
	{
		/* Transition to epoch_2 difficulty levels breaks previous behavior.
		 * When difficulty is not given, the default difficulty to validate changes when the first epoch_2 block is seen, breaking previous behavior.
		 * For this reason, when difficulty is not given, the "valid" field is no longer included in the response to break loudly any client expecting it.
		 * Instead, use the new fields:
		 * * valid_all: the work is valid at the current highest difficulty threshold
		 * * valid_receive: the work is valid for a receive block in an epoch_2 upgraded account
		 */

		auto result_difficulty (node.network_params.work.difficulty (work_version, hash, work));
		if (request.count ("difficulty"))
		{
			response_l.put ("valid", (result_difficulty >= difficulty) ? "1" : "0");
		}
		response_l.put ("valid_all", (result_difficulty >= node.default_difficulty (work_version)) ? "1" : "0");
		response_l.put ("valid_receive", (result_difficulty >= node.network_params.work.threshold (work_version, nano::block_details (nano::epoch::epoch_2, false, true, false))) ? "1" : "0");
		response_l.put ("difficulty", nano::to_string_hex (result_difficulty));
		auto result_multiplier = nano::difficulty::to_multiplier (result_difficulty, node.default_difficulty (work_version));
		response_l.put ("multiplier", nano::to_string (result_multiplier));
	}
	response_errors ();
}

void nano::json_handler::work_peer_add ()
{
	std::string address_text = request.get<std::string> ("address");
	std::string port_text = request.get<std::string> ("port");
	uint16_t port;
	if (!nano::parse_port (port_text, port))
	{
		node.config->work_peers.push_back (std::make_pair (address_text, port));
		response_l.put ("success", "");
	}
	else
	{
		ec = nano::error_common::invalid_port;
	}
	response_errors ();
}

void nano::json_handler::work_peers ()
{
	boost::property_tree::ptree work_peers_l;
	for (auto i (node.config->work_peers.begin ()), n (node.config->work_peers.end ()); i != n; ++i)
	{
		boost::property_tree::ptree entry;
		entry.put ("", boost::str (boost::format ("%1%:%2%") % i->first % i->second));
		work_peers_l.push_back (std::make_pair ("", entry));
	}
	response_l.add_child ("work_peers", work_peers_l);
	response_errors ();
}

void nano::json_handler::work_peers_clear ()
{
	node.config->work_peers.clear ();
	response_l.put ("success", "");
	response_errors ();
}

void nano::json_handler::populate_backlog ()
{
	node.backlog.trigger ();
	response_l.put ("success", "");
	response_errors ();
}

void nano::inprocess_rpc_handler::process_request (std::string const &, std::string const & body_a, std::function<void (std::string const &)> response_a)
{
	// Note that if the rpc action is async, the shared_ptr<json_handler> lifetime will be extended by the action handler
	auto handler (std::make_shared<nano::json_handler> (node, node_rpc_config, body_a, response_a, [this] () {
		this->stop_callback ();
		this->stop ();
	}));
	handler->process_request ();
}

void nano::inprocess_rpc_handler::process_request_v2 (rpc_handler_request_params const & params_a, std::string const & body_a, std::function<void (std::shared_ptr<std::string> const &)> response_a)
{
	std::string body_l = params_a.json_envelope (body_a);
	auto handler (std::make_shared<nano::ipc::flatbuffers_handler> (node, ipc_server, nullptr, node.config->ipc_config));
	handler->process_json (reinterpret_cast<uint8_t const *> (body_l.data ()), body_l.size (), response_a);
}

namespace
{
void construct_json (nano::container_info_component * component, boost::property_tree::ptree & parent)
{
	// We are a leaf node, print name and exit
	if (!component->is_composite ())
	{
		auto & leaf_info = static_cast<nano::container_info_leaf *> (component)->get_info ();
		boost::property_tree::ptree child;
		child.put ("count", leaf_info.count);
		child.put ("size", leaf_info.count * leaf_info.sizeof_element);
		parent.add_child (leaf_info.name, child);
		return;
	}

	auto composite = static_cast<nano::container_info_composite *> (component);

	boost::property_tree::ptree current;
	for (auto & child : composite->get_children ())
	{
		construct_json (child.get (), current);
	}

	parent.add_child (composite->get_name (), current);
}

// Any RPC handlers which require no arguments (excl default arguments) should go here.
// This is to prevent large if/else chains which compilers can have limits for (MSVC for instance has 128).
ipc_json_handler_no_arg_func_map create_ipc_json_handler_no_arg_func_map ()
{
	ipc_json_handler_no_arg_func_map no_arg_funcs;
	no_arg_funcs.emplace ("account_balance", &nano::json_handler::account_balance);
	no_arg_funcs.emplace ("account_block_count", &nano::json_handler::account_block_count);
	no_arg_funcs.emplace ("account_count", &nano::json_handler::account_count);
	no_arg_funcs.emplace ("account_create", &nano::json_handler::account_create);
	no_arg_funcs.emplace ("account_get", &nano::json_handler::account_get);
	no_arg_funcs.emplace ("account_history", &nano::json_handler::account_history);
	no_arg_funcs.emplace ("account_info", &nano::json_handler::account_info);
	no_arg_funcs.emplace ("account_key", &nano::json_handler::account_key);
	no_arg_funcs.emplace ("account_list", &nano::json_handler::account_list);
	no_arg_funcs.emplace ("account_move", &nano::json_handler::account_move);
	no_arg_funcs.emplace ("account_remove", &nano::json_handler::account_remove);
	no_arg_funcs.emplace ("account_representative", &nano::json_handler::account_representative);
	no_arg_funcs.emplace ("account_representative_set", &nano::json_handler::account_representative_set);
	no_arg_funcs.emplace ("account_weight", &nano::json_handler::account_weight);
	no_arg_funcs.emplace ("accounts_balances", &nano::json_handler::accounts_balances);
	no_arg_funcs.emplace ("accounts_representatives", &nano::json_handler::accounts_representatives);
	no_arg_funcs.emplace ("accounts_create", &nano::json_handler::accounts_create);
	no_arg_funcs.emplace ("accounts_frontiers", &nano::json_handler::accounts_frontiers);
	no_arg_funcs.emplace ("accounts_pending", &nano::json_handler::accounts_pending);
	no_arg_funcs.emplace ("accounts_receivable", &nano::json_handler::accounts_receivable);
	no_arg_funcs.emplace ("active_difficulty", &nano::json_handler::active_difficulty);
	no_arg_funcs.emplace ("available_supply", &nano::json_handler::available_supply);
	no_arg_funcs.emplace ("block_info", &nano::json_handler::block_info);
	no_arg_funcs.emplace ("block", &nano::json_handler::block_info);
	no_arg_funcs.emplace ("block_confirm", &nano::json_handler::block_confirm);
	no_arg_funcs.emplace ("blocks", &nano::json_handler::blocks);
	no_arg_funcs.emplace ("blocks_info", &nano::json_handler::blocks_info);
	no_arg_funcs.emplace ("block_account", &nano::json_handler::block_account);
	no_arg_funcs.emplace ("block_count", &nano::json_handler::block_count);
	no_arg_funcs.emplace ("block_create", &nano::json_handler::block_create);
	no_arg_funcs.emplace ("block_hash", &nano::json_handler::block_hash);
	no_arg_funcs.emplace ("bootstrap", &nano::json_handler::bootstrap);
	no_arg_funcs.emplace ("bootstrap_any", &nano::json_handler::bootstrap_any);
	no_arg_funcs.emplace ("bootstrap_lazy", &nano::json_handler::bootstrap_lazy);
	no_arg_funcs.emplace ("bootstrap_status", &nano::json_handler::bootstrap_status);
	no_arg_funcs.emplace ("confirmation_active", &nano::json_handler::confirmation_active);
	no_arg_funcs.emplace ("confirmation_history", &nano::json_handler::confirmation_history);
	no_arg_funcs.emplace ("confirmation_info", &nano::json_handler::confirmation_info);
	no_arg_funcs.emplace ("confirmation_quorum", &nano::json_handler::confirmation_quorum);
	no_arg_funcs.emplace ("database_txn_tracker", &nano::json_handler::database_txn_tracker);
	no_arg_funcs.emplace ("delegators", &nano::json_handler::delegators);
	no_arg_funcs.emplace ("delegators_count", &nano::json_handler::delegators_count);
	no_arg_funcs.emplace ("deterministic_key", &nano::json_handler::deterministic_key);
	no_arg_funcs.emplace ("election_statistics", &nano::json_handler::election_statistics);
	no_arg_funcs.emplace ("frontiers", &nano::json_handler::frontiers);
	no_arg_funcs.emplace ("frontier_count", &nano::json_handler::account_count);
	no_arg_funcs.emplace ("keepalive", &nano::json_handler::keepalive);
	no_arg_funcs.emplace ("key_create", &nano::json_handler::key_create);
	no_arg_funcs.emplace ("key_expand", &nano::json_handler::key_expand);
	no_arg_funcs.emplace ("ledger", &nano::json_handler::ledger);
	no_arg_funcs.emplace ("node_id", &nano::json_handler::node_id);
	no_arg_funcs.emplace ("node_id_delete", &nano::json_handler::node_id_delete);
	no_arg_funcs.emplace ("password_change", &nano::json_handler::password_change);
	no_arg_funcs.emplace ("password_enter", &nano::json_handler::password_enter);
	no_arg_funcs.emplace ("wallet_unlock", &nano::json_handler::password_enter);
	no_arg_funcs.emplace ("peers", &nano::json_handler::peers);
	no_arg_funcs.emplace ("pending", &nano::json_handler::pending);
	no_arg_funcs.emplace ("pending_exists", &nano::json_handler::pending_exists);
	no_arg_funcs.emplace ("receivable", &nano::json_handler::receivable);
	no_arg_funcs.emplace ("receivable_exists", &nano::json_handler::receivable_exists);
	no_arg_funcs.emplace ("process", &nano::json_handler::process);
	no_arg_funcs.emplace ("pruned_exists", &nano::json_handler::pruned_exists);
	no_arg_funcs.emplace ("receive", &nano::json_handler::receive);
	no_arg_funcs.emplace ("receive_minimum", &nano::json_handler::receive_minimum);
	no_arg_funcs.emplace ("receive_minimum_set", &nano::json_handler::receive_minimum_set);
	no_arg_funcs.emplace ("representatives", &nano::json_handler::representatives);
	no_arg_funcs.emplace ("representatives_online", &nano::json_handler::representatives_online);
	no_arg_funcs.emplace ("republish", &nano::json_handler::republish);
	no_arg_funcs.emplace ("search_pending", &nano::json_handler::search_pending);
	no_arg_funcs.emplace ("search_receivable", &nano::json_handler::search_receivable);
	no_arg_funcs.emplace ("search_pending_all", &nano::json_handler::search_pending_all);
	no_arg_funcs.emplace ("search_receivable_all", &nano::json_handler::search_receivable_all);
	no_arg_funcs.emplace ("send", &nano::json_handler::send);
	no_arg_funcs.emplace ("sign", &nano::json_handler::sign);
	no_arg_funcs.emplace ("stats", &nano::json_handler::stats);
	no_arg_funcs.emplace ("stats_clear", &nano::json_handler::stats_clear);
	no_arg_funcs.emplace ("stop", &nano::json_handler::stop);
	no_arg_funcs.emplace ("telemetry", &nano::json_handler::telemetry);
	no_arg_funcs.emplace ("unchecked", &nano::json_handler::unchecked);
	no_arg_funcs.emplace ("unchecked_clear", &nano::json_handler::unchecked_clear);
	no_arg_funcs.emplace ("unchecked_get", &nano::json_handler::unchecked_get);
	no_arg_funcs.emplace ("unchecked_keys", &nano::json_handler::unchecked_keys);
	no_arg_funcs.emplace ("unopened", &nano::json_handler::unopened);
	no_arg_funcs.emplace ("uptime", &nano::json_handler::uptime);
	no_arg_funcs.emplace ("validate_account_number", &nano::json_handler::validate_account_number);
	no_arg_funcs.emplace ("version", &nano::json_handler::version);
	no_arg_funcs.emplace ("wallet_add", &nano::json_handler::wallet_add);
	no_arg_funcs.emplace ("wallet_add_watch", &nano::json_handler::wallet_add_watch);
	no_arg_funcs.emplace ("wallet_balances", &nano::json_handler::wallet_balances);
	no_arg_funcs.emplace ("wallet_change_seed", &nano::json_handler::wallet_change_seed);
	no_arg_funcs.emplace ("wallet_contains", &nano::json_handler::wallet_contains);
	no_arg_funcs.emplace ("wallet_create", &nano::json_handler::wallet_create);
	no_arg_funcs.emplace ("wallet_destroy", &nano::json_handler::wallet_destroy);
	no_arg_funcs.emplace ("wallet_export", &nano::json_handler::wallet_export);
	no_arg_funcs.emplace ("wallet_frontiers", &nano::json_handler::wallet_frontiers);
	no_arg_funcs.emplace ("wallet_history", &nano::json_handler::wallet_history);
	no_arg_funcs.emplace ("wallet_info", &nano::json_handler::wallet_info);
	no_arg_funcs.emplace ("wallet_balance_total", &nano::json_handler::wallet_info);
	no_arg_funcs.emplace ("wallet_key_valid", &nano::json_handler::wallet_key_valid);
	no_arg_funcs.emplace ("wallet_ledger", &nano::json_handler::wallet_ledger);
	no_arg_funcs.emplace ("wallet_lock", &nano::json_handler::wallet_lock);
	no_arg_funcs.emplace ("wallet_pending", &nano::json_handler::wallet_pending);
	no_arg_funcs.emplace ("wallet_receivable", &nano::json_handler::wallet_receivable);
	no_arg_funcs.emplace ("wallet_representative", &nano::json_handler::wallet_representative);
	no_arg_funcs.emplace ("wallet_representative_set", &nano::json_handler::wallet_representative_set);
	no_arg_funcs.emplace ("wallet_republish", &nano::json_handler::wallet_republish);
	no_arg_funcs.emplace ("wallet_work_get", &nano::json_handler::wallet_work_get);
	no_arg_funcs.emplace ("work_generate", &nano::json_handler::work_generate);
	no_arg_funcs.emplace ("work_cancel", &nano::json_handler::work_cancel);
	no_arg_funcs.emplace ("work_get", &nano::json_handler::work_get);
	no_arg_funcs.emplace ("work_set", &nano::json_handler::work_set);
	no_arg_funcs.emplace ("work_validate", &nano::json_handler::work_validate);
	no_arg_funcs.emplace ("work_peer_add", &nano::json_handler::work_peer_add);
	no_arg_funcs.emplace ("work_peers", &nano::json_handler::work_peers);
	no_arg_funcs.emplace ("work_peers_clear", &nano::json_handler::work_peers_clear);
	no_arg_funcs.emplace ("populate_backlog", &nano::json_handler::populate_backlog);
	return no_arg_funcs;
}

/** Due to the asynchronous nature of updating confirmation heights, it can also be necessary to check active roots */
bool block_confirmed (nano::node & node, nano::store::transaction & transaction, nano::block_hash const & hash, bool include_active, bool include_only_confirmed)
{
	bool is_confirmed = false;
	if (include_active && !include_only_confirmed)
	{
		is_confirmed = true;
	}
	// Check whether the confirmation height is set
	else if (node.ledger.confirmed ().block_exists_or_pruned (transaction, hash))
	{
		is_confirmed = true;
	}
	// This just checks it's not currently undergoing an active transaction
	else if (!include_only_confirmed)
	{
		auto block (node.ledger.any ().block_get (transaction, hash));
		is_confirmed = (block != nullptr && !node.active.active (*block));
	}

	return is_confirmed;
}

char const * epoch_as_string (nano::epoch epoch)
{
	switch (epoch)
	{
		case nano::epoch::epoch_2:
			return "2";
		case nano::epoch::epoch_1:
			return "1";
		default:
			return "0";
	}
}
}
