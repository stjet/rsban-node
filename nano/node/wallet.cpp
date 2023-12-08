#include <nano/crypto_lib/random_pool.hpp>
#include <nano/lib/rsnano.hpp>
#include <nano/lib/rsnanoutils.hpp>
#include <nano/lib/threading.hpp>
#include <nano/lib/utility.hpp>
#include <nano/node/election.hpp>
#include <nano/node/node.hpp>
#include <nano/node/wallet.hpp>
#include <nano/store/lmdb/iterator.hpp>

#include <boost/format.hpp>
#include <boost/polymorphic_cast.hpp>
#include <boost/property_tree/json_parser.hpp>

#include <algorithm>
#include <cstdint>
#include <future>
#include <memory>

nano::uint256_union nano::wallet_store::check (store::transaction const & transaction_a)
{
	nano::uint256_union result;
	rsnano::rsn_lmdb_wallet_store_check (rust_handle, transaction_a.get_rust_handle (), result.bytes.data ());
	return result;
}

nano::uint256_union nano::wallet_store::salt (store::transaction const & transaction_a)
{
	nano::uint256_union result;
	rsnano::rsn_lmdb_wallet_store_salt (rust_handle, transaction_a.get_rust_handle (), result.bytes.data ());
	return result;
}

void nano::wallet_store::wallet_key (nano::raw_key & prv_a, store::transaction const & transaction_a)
{
	rsnano::rsn_lmdb_wallet_store_wallet_key (rust_handle, prv_a.bytes.data (), transaction_a.get_rust_handle ());
}

void nano::wallet_store::seed (nano::raw_key & prv_a, store::transaction const & transaction_a)
{
	rsnano::rsn_lmdb_wallet_store_seed (rust_handle, prv_a.bytes.data (), transaction_a.get_rust_handle ());
}

void nano::wallet_store::seed_set (store::transaction const & transaction_a, nano::raw_key const & prv_a)
{
	rsnano::rsn_lmdb_wallet_store_seed_set (rust_handle, transaction_a.get_rust_handle (), prv_a.bytes.data ());
}

nano::public_key nano::wallet_store::deterministic_insert (store::transaction const & transaction_a)
{
	nano::public_key key;
	rsnano::rsn_lmdb_wallet_store_deterministic_insert (rust_handle, transaction_a.get_rust_handle (), key.bytes.data ());
	return key;
}

nano::public_key nano::wallet_store::deterministic_insert (store::transaction const & transaction_a, uint32_t const index)
{
	nano::public_key key;
	rsnano::rsn_lmdb_wallet_store_deterministic_insert_at (rust_handle, transaction_a.get_rust_handle (), index, key.bytes.data ());
	return key;
}

nano::raw_key nano::wallet_store::deterministic_key (store::transaction const & transaction_a, uint32_t index_a)
{
	nano::raw_key key;
	rsnano::rsn_lmdb_wallet_store_deterministic_key (rust_handle, transaction_a.get_rust_handle (), index_a, key.bytes.data ());
	return key;
}

uint32_t nano::wallet_store::deterministic_index_get (store::transaction const & transaction_a)
{
	return rsnano::rsn_lmdb_wallet_store_deterministic_index_get (rust_handle, transaction_a.get_rust_handle ());
}

void nano::wallet_store::deterministic_index_set (store::transaction const & transaction_a, uint32_t index_a)
{
	rsnano::rsn_lmdb_wallet_store_deterministic_index_set (rust_handle, transaction_a.get_rust_handle (), index_a);
}

void nano::wallet_store::deterministic_clear (store::transaction const & transaction_a)
{
	rsnano::rsn_lmdb_wallet_store_deterministic_clear (rust_handle, transaction_a.get_rust_handle ());
}

bool nano::wallet_store::valid_password (store::transaction const & transaction_a)
{
	return rsnano::rsn_lmdb_wallet_store_valid_password (rust_handle, transaction_a.get_rust_handle ());
}

bool nano::wallet_store::attempt_password (store::transaction const & transaction_a, std::string const & password_a)
{
	return !rsnano::rsn_lmdb_wallet_store_attempt_password (rust_handle, transaction_a.get_rust_handle (), password_a.c_str ());
}

bool nano::wallet_store::rekey (store::transaction const & transaction_a, std::string const & password_a)
{
	return !rsnano::rsn_lmdb_wallet_store_rekey (rust_handle, transaction_a.get_rust_handle (), password_a.c_str ());
}

void nano::wallet_store::derive_key (nano::raw_key & prv_a, store::transaction const & transaction_a, std::string const & password_a)
{
	rsnano::rsn_lmdb_wallet_store_derive_key (rust_handle, prv_a.bytes.data (), transaction_a.get_rust_handle (), password_a.c_str ());
}

int const nano::wallet_store::special_count (7);

nano::wallet_store::wallet_store (bool & init_a, nano::kdf & kdf_a, store::transaction & transaction_a, nano::account representative_a, unsigned fanout_a, std::string const & wallet_a, std::string const & json_a) :
	kdf (kdf_a),
	rust_handle{ rsnano::rsn_lmdb_wallet_store_create2 (fanout_a, kdf_a.handle, transaction_a.get_rust_handle (), wallet_a.c_str (), json_a.c_str ()) }
{
	init_a = rust_handle == nullptr;
}

nano::wallet_store::wallet_store (bool & init_a, nano::kdf & kdf_a, store::transaction & transaction_a, nano::account representative_a, unsigned fanout_a, std::string const & wallet_a) :
	kdf (kdf_a),
	rust_handle{ rsnano::rsn_lmdb_wallet_store_create (fanout_a, kdf_a.handle, transaction_a.get_rust_handle (), representative_a.bytes.data (), wallet_a.c_str ()) }
{
	init_a = rust_handle == nullptr;
}

nano::wallet_store::~wallet_store ()
{
	if (rust_handle != nullptr)
		rsnano::rsn_lmdb_wallet_store_destroy (rust_handle);
}

bool nano::wallet_store::is_open () const
{
	return rsnano::rsn_lmdb_wallet_store_is_open (rust_handle);
}

void nano::wallet_store::lock ()
{
	rsnano::rsn_lmdb_wallet_store_lock (rust_handle);
}

void nano::wallet_store::password (nano::raw_key & password_a) const
{
	rsnano::rsn_lmdb_wallet_store_password (rust_handle, password_a.bytes.data ());
}

void nano::wallet_store::set_password (nano::raw_key const & password_a)
{
	rsnano::rsn_lmdb_wallet_store_set_password (rust_handle, password_a.bytes.data ());
}

std::vector<nano::account> nano::wallet_store::accounts (store::transaction const & transaction_a)
{
	rsnano::U256ArrayDto dto;
	rsnano::rsn_lmdb_wallet_store_accounts (rust_handle, transaction_a.get_rust_handle (), &dto);
	std::vector<nano::account> result;
	result.reserve (dto.count);
	for (int i = 0; i < dto.count; ++i)
	{
		nano::account account;
		std::copy (std::begin (dto.items[i]), std::end (dto.items[i]), std::begin (account.bytes));
		result.push_back (account);
	}
	rsnano::rsn_u256_array_destroy (&dto);
	return result;
}

bool nano::wallet_store::is_representative (store::transaction const & transaction_a)
{
	return exists (transaction_a, representative (transaction_a));
}

void nano::wallet_store::representative_set (store::transaction const & transaction_a, nano::account const & representative_a)
{
	rsnano::rsn_lmdb_wallet_store_representative_set (rust_handle, transaction_a.get_rust_handle (), representative_a.bytes.data ());
}

nano::account nano::wallet_store::representative (store::transaction const & transaction_a)
{
	nano::account rep;
	rsnano::rsn_lmdb_wallet_store_representative (rust_handle, transaction_a.get_rust_handle (), rep.bytes.data ());
	return rep;
}

nano::public_key nano::wallet_store::insert_adhoc (store::transaction const & transaction_a, nano::raw_key const & prv)
{
	nano::public_key pub;
	rsnano::rsn_lmdb_wallet_store_insert_adhoc (rust_handle, transaction_a.get_rust_handle (), prv.bytes.data (), pub.bytes.data ());
	return pub;
}

bool nano::wallet_store::insert_watch (store::transaction const & transaction_a, nano::account const & pub_a)
{
	return !rsnano::rsn_lmdb_wallet_store_insert_watch (rust_handle, transaction_a.get_rust_handle (), pub_a.bytes.data ());
}

void nano::wallet_store::erase (store::transaction const & transaction_a, nano::account const & pub)
{
	rsnano::rsn_lmdb_wallet_store_erase (rust_handle, transaction_a.get_rust_handle (), pub.bytes.data ());
}

nano::key_type nano::wallet_store::key_type (nano::wallet_value const & value_a)
{
	auto dto{ value_a.to_dto () };
	return static_cast<nano::key_type> (rsnano::rsn_lmdb_wallet_store_key_type (&dto));
}

bool nano::wallet_store::fetch (store::transaction const & transaction_a, nano::account const & pub, nano::raw_key & prv)
{
	return !rsnano::rsn_lmdb_wallet_store_fetch (rust_handle, transaction_a.get_rust_handle (), pub.bytes.data (), prv.bytes.data ());
}

bool nano::wallet_store::exists (store::transaction const & transaction_a, nano::public_key const & pub)
{
	return rsnano::rsn_lmdb_wallet_store_exists (rust_handle, transaction_a.get_rust_handle (), pub.bytes.data ());
}

void nano::wallet_store::serialize_json (store::transaction const & transaction_a, std::string & string_a)
{
	rsnano::StringDto dto;
	rsnano::rsn_lmdb_wallet_store_serialize_json (rust_handle, transaction_a.get_rust_handle (), &dto);
	string_a = rsnano::convert_dto_to_string (dto);
}

void nano::wallet_store::write_backup (store::transaction const & transaction_a, std::filesystem::path const & path_a)
{
	rsnano::rsn_lmdb_wallet_store_write_backup (rust_handle, transaction_a.get_rust_handle (), path_a.c_str ());
}

bool nano::wallet_store::move (store::transaction const & transaction_a, nano::wallet_store & other_a, std::vector<nano::public_key> const & keys)
{
	return !rsnano::rsn_lmdb_wallet_store_move (rust_handle, transaction_a.get_rust_handle (), other_a.rust_handle, reinterpret_cast<const uint8_t *> (keys.data ()), keys.size ());
}

bool nano::wallet_store::import (store::transaction const & transaction_a, nano::wallet_store & other_a)
{
	return !rsnano::rsn_lmdb_wallet_store_import (rust_handle, transaction_a.get_rust_handle (), other_a.rust_handle);
}

bool nano::wallet_store::work_get (store::transaction const & transaction_a, nano::public_key const & pub_a, uint64_t & work_a)
{
	return !rsnano::rsn_lmdb_wallet_store_work_get (rust_handle, transaction_a.get_rust_handle (), pub_a.bytes.data (), &work_a);
}

void nano::wallet_store::work_put (store::transaction const & transaction_a, nano::public_key const & pub_a, uint64_t work_a)
{
	rsnano::rsn_lmdb_wallet_store_work_put (rust_handle, transaction_a.get_rust_handle (), pub_a.bytes.data (), work_a);
}

unsigned nano::wallet_store::version (store::transaction const & transaction_a)
{
	return rsnano::rsn_lmdb_wallet_store_version (rust_handle, transaction_a.get_rust_handle ());
}

nano::kdf::kdf (unsigned kdf_work) :
	handle{ rsnano::rsn_kdf_create (kdf_work) }
{
}

nano::kdf::~kdf ()
{
	rsnano::rsn_kdf_destroy (handle);
}

void nano::kdf::phs (nano::raw_key & result_a, std::string const & password_a, nano::uint256_union const & salt_a)
{
	rsnano::rsn_kdf_phs (handle, result_a.bytes.data (), password_a.c_str (), salt_a.bytes.data ());
}

nano::wallet_action_thread::wallet_action_thread () :
	observer ([] (bool) {}),
	stopped (false)
{
}

void nano::wallet_action_thread::start ()
{
	thread = std::thread{ [this] () {
		nano::thread_role::set (nano::thread_role::name::wallet_actions);
		do_wallet_actions ();
	} };
}

void nano::wallet_action_thread::stop ()
{
	{
		nano::lock_guard<nano::mutex> action_lock{ action_mutex };
		stopped = true;
		actions.clear ();
	}
	condition.notify_all ();
	if (thread.joinable ())
	{
		thread.join ();
	}
}

void nano::wallet_action_thread::queue_wallet_action (nano::uint128_t const & amount_a, std::shared_ptr<nano::wallet> const & wallet_a, std::function<void (nano::wallet &)> action_a)
{
	{
		nano::lock_guard<nano::mutex> action_lock{ action_mutex };
		actions.emplace (amount_a, std::make_pair (wallet_a, action_a));
	}
	condition.notify_all ();
}

nano::lock_guard<nano::mutex> nano::wallet_action_thread::lock ()
{
	return nano::lock_guard<nano::mutex>{ action_mutex };
}

size_t nano::wallet_action_thread::size ()
{
	nano::lock_guard<nano::mutex> action_lock{ action_mutex };
	return actions.size ();
}

void nano::wallet_action_thread::do_wallet_actions ()
{
	nano::unique_lock<nano::mutex> action_lock{ action_mutex };
	while (!stopped)
	{
		if (!actions.empty ())
		{
			auto first (actions.begin ());
			auto wallet (first->second.first);
			auto current (std::move (first->second.second));
			actions.erase (first);
			if (wallet->live ())
			{
				action_lock.unlock ();
				observer (true);
				current (*wallet);
				observer (false);
				action_lock.lock ();
			}
		}
		else
		{
			condition.wait (action_lock);
		}
	}
}

nano::wallet::representatives_lock::representatives_lock (rsnano::RepresentativesLockHandle * handle) :
	handle{ handle }
{
}

nano::wallet::representatives_lock::~representatives_lock ()
{
	rsnano::rsn_representatives_lock_destroy (handle);
}

size_t nano::wallet::representatives_lock::size () const
{
	return rsnano::rsn_representatives_lock_size (handle);
}

void nano::wallet::representatives_lock::insert (nano::public_key const & rep)
{
	rsnano::rsn_representatives_lock_insert (handle, rep.bytes.data ());
}

std::unordered_set<nano::account> nano::wallet::representatives_lock::get_all ()
{
	auto vec_handle = rsnano::rsn_representatives_lock_get_all (handle);
	std::unordered_set<nano::account> result{};
	auto len = rsnano::rsn_account_vec_len (vec_handle);
	for (auto i = 0; i < len; ++i)
	{
		nano::account rep;
		rsnano::rsn_account_vec_get (vec_handle, i, rep.bytes.data ());
		result.insert (rep);
	}
	rsnano::rsn_account_vec_destroy (vec_handle);
	return result;
}

void nano::wallet::representatives_lock::set (std::unordered_set<nano::account> reps)
{
	rsnano::rsn_representatives_lock_clear (handle);
	for (const auto & rep : reps)
	{
		insert (rep);
	}
}

nano::wallet::representatives_mutex::representatives_mutex (rsnano::WalletHandle * handle) :
	handle{ handle }
{
}

nano::wallet::representatives_lock nano::wallet::representatives_mutex::lock ()
{
	return { rsnano::rsn_representatives_lock_create (handle) };
}

nano::wallet::wallet (bool & init_a, store::transaction & transaction_a, nano::wallets & wallets_a, std::string const & wallet_a) :
	store (init_a, wallets_a.kdf, transaction_a, wallets_a.node.config->random_representative (), wallets_a.node.config->password_fanout, wallet_a),
	wallets (wallets_a),
	wallet_actions{ wallets_a.wallet_actions },
	representatives{ wallets_a.representatives },
	node{ wallets_a.node },
	env (boost::polymorphic_downcast<nano::mdb_wallets_store *> (wallets_a.node.wallets_store_impl.get ())->environment),
	handle{ rsnano::rsn_wallet_create () },
	representatives_mutex{ handle }
{
}

nano::wallet::wallet (bool & init_a, store::transaction & transaction_a, nano::wallets & wallets_a, std::string const & wallet_a, std::string const & json) :
	store (init_a, wallets_a.kdf, transaction_a, wallets_a.node.config->random_representative (), wallets_a.node.config->password_fanout, wallet_a, json),
	wallets (wallets_a),
	wallet_actions{ wallets_a.wallet_actions },
	representatives{ wallets_a.representatives },
	node{ wallets_a.node },
	env (boost::polymorphic_downcast<nano::mdb_wallets_store *> (wallets_a.node.wallets_store_impl.get ())->environment),
	handle{ rsnano::rsn_wallet_create () },
	representatives_mutex{ handle }
{
}

nano::wallet::~wallet ()
{
	rsnano::rsn_wallet_destroy (handle);
}

void nano::wallet::enter_initial_password ()
{
	nano::raw_key password_l;
	store.password (password_l);
	if (password_l.is_zero ())
	{
		auto transaction (env.tx_begin_write ());
		if (store.valid_password (*transaction))
		{
			// Newly created wallets have a zero key
			store.rekey (*transaction, "");
		}
		else
		{
			enter_password (*transaction, "");
		}
	}
}

bool nano::wallet::enter_password (store::transaction const & transaction_a, std::string const & password_a)
{
	auto error (store.attempt_password (transaction_a, password_a));
	if (!error)
	{
		auto this_l (shared_from_this ());
		node.background ([this_l] () {
			auto tx{ this_l->env.tx_begin_read () };
			this_l->search_receivable (*tx);
		});
		node.logger->try_log ("Wallet unlocked");
	}
	else
	{
		node.logger->try_log ("Invalid password, wallet locked");
	}
	return error;
}

nano::public_key nano::wallet::deterministic_insert (store::transaction const & transaction_a, bool generate_work_a)
{
	nano::public_key key{};
	if (store.valid_password (transaction_a))
	{
		key = store.deterministic_insert (transaction_a);
		if (generate_work_a)
		{
			work_ensure (key, key);
		}
		auto half_principal_weight (node.minimum_principal_weight () / 2);
		if (wallets.representatives.check_rep (key, half_principal_weight))
		{
			auto lock{ representatives_mutex.lock () };
			lock.insert (key);
		}
	}
	return key;
}

nano::public_key nano::wallet::deterministic_insert (bool generate_work_a)
{
	auto transaction (env.tx_begin_write ());
	auto result (deterministic_insert (*transaction, generate_work_a));
	return result;
}

nano::public_key nano::wallet::insert_adhoc (nano::raw_key const & key_a, bool generate_work_a)
{
	nano::public_key key{};
	auto transaction (env.tx_begin_write ());
	if (store.valid_password (*transaction))
	{
		key = store.insert_adhoc (*transaction, key_a);
		auto block_transaction (node.store.tx_begin_read ());
		if (generate_work_a)
		{
			work_ensure (key, node.ledger.latest_root (*block_transaction, key));
		}
		auto half_principal_weight (node.minimum_principal_weight () / 2);
		// Makes sure that the representatives container will
		// be in sync with any added keys.
		transaction->commit ();
		if (representatives.check_rep (key, half_principal_weight))
		{
			auto lock{ representatives_mutex.lock () };
			lock.insert (key);
		}
	}
	return key;
}

bool nano::wallet::insert_watch (store::transaction const & transaction_a, nano::public_key const & pub_a)
{
	return store.insert_watch (transaction_a, pub_a);
}

bool nano::wallet::exists (nano::public_key const & account_a)
{
	auto transaction (env.tx_begin_read ());
	return store.exists (*transaction, account_a);
}

bool nano::wallet::import (std::string const & json_a, std::string const & password_a)
{
	auto error (false);
	std::unique_ptr<nano::wallet_store> temp;
	{
		auto transaction (env.tx_begin_write ());
		nano::uint256_union id;
		random_pool::generate_block (id.bytes.data (), id.bytes.size ());
		temp = std::make_unique<nano::wallet_store> (error, wallets.kdf, *transaction, 0, 1, id.to_string (), json_a);
	}
	if (!error)
	{
		auto transaction (env.tx_begin_write ());
		error = temp->attempt_password (*transaction, password_a);
	}
	auto transaction (env.tx_begin_write ());
	if (!error)
	{
		error = store.import (*transaction, *temp);
	}
	temp->destroy (*transaction);
	return error;
}

void nano::wallet::serialize (std::string & json_a)
{
	auto transaction (env.tx_begin_read ());
	store.serialize_json (*transaction, json_a);
}

void nano::wallet_store::destroy (store::transaction const & transaction_a)
{
	if (rust_handle != nullptr)
		rsnano::rsn_lmdb_wallet_store_destroy2 (rust_handle, transaction_a.get_rust_handle ());
}

std::shared_ptr<nano::block> nano::wallet::receive_action (nano::block_hash const & send_hash_a, nano::account const & representative_a, nano::uint128_union const & amount_a, nano::account const & account_a, uint64_t work_a, bool generate_work_a)
{
	std::shared_ptr<nano::block> block;
	nano::epoch epoch = nano::epoch::epoch_0;
	if (node.config->receive_minimum.number () <= amount_a.number ())
	{
		auto block_transaction (node.ledger.store.tx_begin_read ());
		auto transaction (env.tx_begin_read ());
		if (node.ledger.block_or_pruned_exists (*block_transaction, send_hash_a))
		{
			auto pending_info = node.ledger.pending_info (*block_transaction, nano::pending_key (account_a, send_hash_a));
			if (pending_info)
			{
				nano::raw_key prv;
				if (!store.fetch (*transaction, account_a, prv))
				{
					if (work_a == 0)
					{
						store.work_get (*transaction, account_a, work_a);
					}
					auto info = node.ledger.account_info (*block_transaction, account_a);
					if (info)
					{
						block = std::make_shared<nano::state_block> (account_a, info->head (), info->representative (), info->balance ().number () + pending_info->amount.number (), send_hash_a, prv, account_a, work_a);
						epoch = std::max (info->epoch (), pending_info->epoch);
					}
					else
					{
						block = std::make_shared<nano::state_block> (account_a, 0, representative_a, pending_info->amount, reinterpret_cast<nano::link const &> (send_hash_a), prv, account_a, work_a);
						epoch = pending_info->epoch;
					}
				}
				else
				{
					node.logger->try_log ("Unable to receive, wallet locked");
				}
			}
			else
			{
				// Ledger doesn't have this marked as available to receive anymore
			}
		}
		else
		{
			// Ledger doesn't have this block anymore.
		}
	}
	else
	{
		node.logger->try_log (boost::str (boost::format ("Not receiving block %1% due to minimum receive threshold") % send_hash_a.to_string ()));
		// Someone sent us something below the threshold of receiving
	}
	if (block != nullptr)
	{
		auto details = nano::block_details (epoch, false, true, false);
		if (action_complete (block, account_a, generate_work_a, details))
		{
			// Return null block after work generation or ledger process error
			block = nullptr;
		}
	}
	return block;
}

std::shared_ptr<nano::block> nano::wallet::change_action (nano::account const & source_a, nano::account const & representative_a, uint64_t work_a, bool generate_work_a)
{
	auto epoch = nano::epoch::epoch_0;
	std::shared_ptr<nano::block> block;
	{
		auto transaction (env.tx_begin_read ());
		auto block_transaction (node.store.tx_begin_read ());
		if (store.valid_password (*transaction))
		{
			auto existing (store.find (*transaction, source_a));
			if (existing != store.end () && !node.ledger.latest (*block_transaction, source_a).is_zero ())
			{
				auto info = node.ledger.account_info (*block_transaction, source_a);
				debug_assert (info);
				nano::raw_key prv;
				auto error2 (store.fetch (*transaction, source_a, prv));
				(void)error2;
				debug_assert (!error2);
				if (work_a == 0)
				{
					store.work_get (*transaction, source_a, work_a);
				}
				block = std::make_shared<nano::state_block> (source_a, info->head (), representative_a, info->balance (), 0, prv, source_a, work_a);
				epoch = info->epoch ();
			}
		}
	}
	if (block != nullptr)
	{
		auto details = nano::block_details (epoch, false, false, false);
		if (action_complete (block, source_a, generate_work_a, details))
		{
			// Return null block after work generation or ledger process error
			block = nullptr;
		}
	}
	return block;
}

std::shared_ptr<nano::block> nano::wallet::send_action (nano::account const & source_a, nano::account const & account_a, nano::uint128_t const & amount_a, uint64_t work_a, bool generate_work_a, boost::optional<std::string> id_a)
{
	auto prepare_send = [&id_a, &node = this->node, &wallets = this->wallets, &store = this->store, &source_a, &amount_a, &work_a, &account_a] (auto const & transaction) {
		auto block_transaction (node.store.tx_begin_read ());

		auto error (false);
		std::shared_ptr<nano::block> block;
		if (id_a)
		{
			auto hash{ wallets.get_block_hash (error, *transaction, *id_a) };
			if (!hash.is_zero ())
			{
				block = node.store.block ().get (*block_transaction, hash);
			}
		}

		nano::block_details details = nano::block_details (nano::epoch::epoch_0, true, false, false);
		auto cached_block (false);
		if (block != nullptr)
		{
			cached_block = true;
			node.network->flood_block (block, nano::transport::buffer_drop_policy::no_limiter_drop);
		}
		if (!error && block == nullptr)
		{
			if (store.valid_password (*transaction))
			{
				auto existing (store.find (*transaction, source_a));
				if (existing != store.end ())
				{
					auto balance (node.ledger.account_balance (*block_transaction, source_a));
					if (!balance.is_zero () && balance >= amount_a)
					{
						auto info = node.ledger.account_info (*block_transaction, source_a);
						debug_assert (info);
						nano::raw_key prv;
						auto error2 (store.fetch (*transaction, source_a, prv));
						(void)error2;
						debug_assert (!error2);
						if (work_a == 0)
						{
							store.work_get (*transaction, source_a, work_a);
						}
						block = std::make_shared<nano::state_block> (source_a, info->head (), info->representative (), balance - amount_a, account_a, prv, source_a, work_a);
						details = nano::block_details (info->epoch (), details.is_send (), details.is_receive (), details.is_epoch ());
						if (id_a && block != nullptr)
						{
							error = wallets.set_block_hash (*transaction, *id_a, block->hash ());
							if (error)
							{
								block = nullptr;
							}
						}
					}
				}
			}
		}
		return std::make_tuple (block, error, cached_block, details);
	};

	std::tuple<std::shared_ptr<nano::block>, bool, bool, nano::block_details> result;
	{
		if (id_a)
		{
			result = prepare_send (env.tx_begin_write ());
		}
		else
		{
			result = prepare_send (env.tx_begin_read ());
		}
	}

	std::shared_ptr<nano::block> block;
	bool error;
	bool cached_block;
	nano::block_details details;
	std::tie (block, error, cached_block, details) = result;

	if (!error && block != nullptr && !cached_block)
	{
		if (action_complete (block, source_a, generate_work_a, details))
		{
			// Return null block after work generation or ledger process error
			block = nullptr;
		}
	}
	return block;
}

bool nano::wallet::action_complete (std::shared_ptr<nano::block> const & block_a, nano::account const & account_a, bool const generate_work_a, nano::block_details const & details_a)
{
	bool error{ false };
	// Unschedule any work caching for this account
	wallets.delayed_work->erase (account_a);
	if (block_a != nullptr)
	{
		auto required_difficulty{ node.network_params.work.threshold (block_a->work_version (), details_a) };
		if (node.network_params.work.difficulty (*block_a) < required_difficulty)
		{
			node.logger->try_log (boost::str (boost::format ("Cached or provided work for block %1% account %2% is invalid, regenerating") % block_a->hash ().to_string () % account_a.to_account ()));
			debug_assert (required_difficulty <= node.max_work_generate_difficulty (block_a->work_version ()));
			error = !node.work_generate_blocking (*block_a, required_difficulty).is_initialized ();
		}
		if (!error)
		{
			auto result = node.process_local (block_a);
			error = !result || result.value ().code != nano::process_result::progress;
			debug_assert (error || block_a->sideband ().details () == details_a);
		}
		if (!error && generate_work_a)
		{
			// Pregenerate work for next block based on the block just created
			work_ensure (account_a, block_a->hash ());
		}
	}
	return error;
}

bool nano::wallet::change_sync (nano::account const & source_a, nano::account const & representative_a)
{
	std::promise<bool> result;
	std::future<bool> future = result.get_future ();
	change_async (
	source_a, representative_a, [&result] (std::shared_ptr<nano::block> const & block_a) {
		result.set_value (block_a == nullptr);
	},
	true);
	return future.get ();
}

void nano::wallet::change_async (nano::account const & source_a, nano::account const & representative_a, std::function<void (std::shared_ptr<nano::block> const &)> const & action_a, uint64_t work_a, bool generate_work_a)
{
	auto this_l (shared_from_this ());
	wallet_actions.queue_wallet_action (nano::wallets::high_priority, this_l, [this_l, source_a, representative_a, action_a, work_a, generate_work_a] (nano::wallet & wallet_a) {
		auto block (wallet_a.change_action (source_a, representative_a, work_a, generate_work_a));
		action_a (block);
	});
}

bool nano::wallet::receive_sync (std::shared_ptr<nano::block> const & block_a, nano::account const & representative_a, nano::uint128_t const & amount_a)
{
	std::promise<bool> result;
	std::future<bool> future = result.get_future ();
	auto destination (block_a->link ().is_zero () ? block_a->destination () : block_a->link ().as_account ());
	receive_async (
	block_a->hash (), representative_a, amount_a, destination, [&result] (std::shared_ptr<nano::block> const & block_a) {
		result.set_value (block_a == nullptr);
	},
	true);
	return future.get ();
}

void nano::wallet::receive_async (nano::block_hash const & hash_a, nano::account const & representative_a, nano::uint128_t const & amount_a, nano::account const & account_a, std::function<void (std::shared_ptr<nano::block> const &)> const & action_a, uint64_t work_a, bool generate_work_a)
{
	auto this_l (shared_from_this ());
	wallet_actions.queue_wallet_action (amount_a, this_l, [this_l, hash_a, representative_a, amount_a, account_a, action_a, work_a, generate_work_a] (nano::wallet & wallet_a) {
		auto block (wallet_a.receive_action (hash_a, representative_a, amount_a, account_a, work_a, generate_work_a));
		action_a (block);
	});
}

void nano::wallet::send_async (nano::account const & source_a, nano::account const & account_a, nano::uint128_t const & amount_a, std::function<void (std::shared_ptr<nano::block> const &)> const & action_a, uint64_t work_a, bool generate_work_a, boost::optional<std::string> id_a)
{
	auto this_l (shared_from_this ());
	wallet_actions.queue_wallet_action (nano::wallets::high_priority, this_l, [this_l, source_a, account_a, amount_a, action_a, work_a, generate_work_a, id_a] (nano::wallet & wallet_a) {
		auto block (wallet_a.send_action (source_a, account_a, amount_a, work_a, generate_work_a, id_a));
		action_a (block);
	});
}

// Update work for account if latest root is root_a
void nano::wallet::work_update (store::transaction const & transaction_a, nano::account const & account_a, nano::root const & root_a, uint64_t work_a)
{
	debug_assert (!node.network_params.work.validate_entry (nano::work_version::work_1, root_a, work_a));
	debug_assert (store.exists (transaction_a, account_a));
	auto block_transaction (node.store.tx_begin_read ());
	auto latest (node.ledger.latest_root (*block_transaction, account_a));
	if (latest == root_a)
	{
		store.work_put (transaction_a, account_a, work_a);
	}
	else
	{
		node.logger->try_log ("Cached work no longer valid, discarding");
	}
}

void nano::wallet::work_ensure (nano::account const & account_a, nano::root const & root_a)
{
	using namespace std::chrono_literals;
	std::chrono::seconds const precache_delay = node.network_params.network.is_dev_network () ? 1s : 10s;

	wallets.delayed_work->operator[] (account_a) = root_a;

	node.workers->add_timed_task (std::chrono::steady_clock::now () + precache_delay, [this_l = shared_from_this (), account_a, root_a] {
		auto delayed_work = this_l->wallets.delayed_work.lock ();
		auto existing (delayed_work->find (account_a));
		if (existing != delayed_work->end () && existing->second == root_a)
		{
			delayed_work->erase (existing);
			this_l->wallet_actions.queue_wallet_action (nano::wallets::generate_priority, this_l, [account_a, root_a] (nano::wallet & wallet_a) {
				wallet_a.work_cache_blocking (account_a, root_a);
			});
		}
	});
}

bool nano::wallet::search_receivable (store::transaction const & wallet_transaction_a)
{
	auto error (!store.valid_password (wallet_transaction_a));
	if (!error)
	{
		for (auto i (store.begin (wallet_transaction_a)), n (store.end ()); i != n; ++i)
		{
			auto block_transaction (node.store.tx_begin_read ());
			nano::account const & account (i->first);
			// Don't search pending for watch-only accounts
			if (!nano::wallet_value (i->second).key.is_zero ())
			{
				for (auto j (node.store.pending ().begin (*block_transaction, nano::pending_key (account, 0))), k (node.store.pending ().end ()); j != k && nano::pending_key (j->first).account == account; ++j)
				{
					nano::pending_key key (j->first);
					auto hash (key.hash);
					nano::pending_info pending (j->second);
					auto amount (pending.amount.number ());
					if (node.config->receive_minimum.number () <= amount)
					{
						node.logger->try_log (boost::str (boost::format ("Found a receivable block %1% for account %2%") % hash.to_string () % pending.source.to_account ()));
						if (node.ledger.block_confirmed (*block_transaction, hash))
						{
							auto representative = store.representative (wallet_transaction_a);
							// Receive confirmed block
							receive_async (hash, representative, amount, account, [] (std::shared_ptr<nano::block> const &) {});
						}
						else if (!node.confirmation_height_processor.is_processing_block (hash))
						{
							auto block (node.store.block ().get (*block_transaction, hash));
							if (block)
							{
								// Request confirmation for block which is not being processed yet
								node.start_election (block);
							}
						}
					}
				}
			}
		}
		node.logger->try_log ("Receivable block search phase completed");
	}
	else
	{
		node.logger->try_log ("Stopping search, wallet is locked");
	}
	return error;
}

uint32_t nano::wallet::deterministic_check (store::transaction const & transaction_a, uint32_t index)
{
	auto block_transaction (node.store.tx_begin_read ());
	for (uint32_t i (index + 1), n (index + 64); i < n; ++i)
	{
		auto prv = store.deterministic_key (transaction_a, i);
		nano::keypair pair (prv.to_string ());
		// Check if account received at least 1 block
		auto latest (node.ledger.latest (*block_transaction, pair.pub));
		if (!latest.is_zero ())
		{
			index = i;
			// i + 64 - Check additional 64 accounts
			// i/64 - Check additional accounts for large wallets. I.e. 64000/64 = 1000 accounts to check
			n = i + 64 + (i / 64);
		}
		else
		{
			// Check if there are pending blocks for account
			for (auto ii (node.store.pending ().begin (*block_transaction, nano::pending_key (pair.pub, 0))), nn (node.store.pending ().end ()); ii != nn && nano::pending_key (ii->first).account == pair.pub; ++ii)
			{
				index = i;
				n = i + 64 + (i / 64);
				break;
			}
		}
	}
	return index;
}

nano::public_key nano::wallet::change_seed (store::transaction const & transaction_a, nano::raw_key const & prv_a, uint32_t count)
{
	store.seed_set (transaction_a, prv_a);
	auto account = deterministic_insert (transaction_a);
	if (count == 0)
	{
		count = deterministic_check (transaction_a, 0);
	}
	for (uint32_t i (0); i < count; ++i)
	{
		// Disable work generation to prevent weak CPU nodes stuck
		account = deterministic_insert (transaction_a, false);
	}
	return account;
}

bool nano::wallet::live ()
{
	return store.is_open ();
}

void nano::wallet::work_cache_blocking (nano::account const & account_a, nano::root const & root_a)
{
	if (node.work_generation_enabled ())
	{
		auto difficulty (node.default_difficulty (nano::work_version::work_1));
		auto opt_work_l (node.work_generate_blocking (nano::work_version::work_1, root_a, difficulty, account_a));
		if (opt_work_l.is_initialized ())
		{
			auto transaction_l (env.tx_begin_write ());
			if (live () && store.exists (*transaction_l, account_a))
			{
				work_update (*transaction_l, account_a, root_a, *opt_work_l);
			}
		}
		else if (!node.stopped)
		{
			node.logger->try_log (boost::str (boost::format ("Could not precache work for root %1% due to work generation failure") % root_a.to_string ()));
		}
	}
}

bool nano::wallet_representatives::check_rep (nano::account const & account_a, nano::uint128_t const & half_principal_weight_a, bool const acquire_lock_a)
{
	auto weight = node.ledger.weight (account_a);

	if (weight < node.config->vote_minimum.number ())
	{
		return false; // account not a representative
	}

	nano::unique_lock<nano::mutex> lock;
	if (acquire_lock_a)
	{
		lock = nano::unique_lock<nano::mutex>{ reps_cache_mutex };
	}

	if (weight >= half_principal_weight_a)
	{
		half_principal = true;
	}

	auto insert_result = accounts.insert (account_a);
	if (!insert_result.second)
	{
		return false; // account already exists
	}

	++voting;

	return true;
}

nano::wallets::wallets_mutex_lock::wallets_mutex_lock (rsnano::WalletsMutexLockHandle * handle) :
	handle{ handle }
{
}

nano::wallets::wallets_mutex_lock::wallets_mutex_lock (nano::wallets::wallets_mutex_lock && other) :
	handle{ other.handle }
{
	other.handle = nullptr;
}

nano::wallets::wallets_mutex_lock::~wallets_mutex_lock ()
{
	if (handle != nullptr)
		rsnano::rsn_lmdb_wallets_mutex_lock_destroy (handle);
}

//std::shared_ptr<nano::wallet> nano::wallets::wallets_mutex_lock::find (nano::wallet_id const & wallet_id){
//	rsnano::WalletHandle * wallet_handle = nullptr;
//	std::shared_ptr<nano::wallet> wallet {};
//	if (rsnano::rsn_lmdb_wallets_mutex_lock_find(handle, wallet_id.bytes.data(), &wallet_handle)){
//		wallet = make_shared<nano::wallet>(wallet_handle);
//	}
//	return wallet;
//}

nano::wallets::wallets_mutex::wallets_mutex (rsnano::LmdbWalletsHandle * handle) :
	handle{ handle }
{
}

nano::wallets::wallets_mutex_lock nano::wallets::wallets_mutex::lock ()
{
	return { rsnano::rsn_lmdb_wallets_mutex_lock (handle) };
}

boost::optional<nano::wallets::wallets_mutex_lock> nano::wallets::wallets_mutex::try_lock ()
{
	auto lock_handle = rsnano::rsn_lmdb_wallets_mutex_try_lock (handle);
	if (lock_handle != nullptr)
	{
		return { nano::wallets::wallets_mutex_lock{ lock_handle } };
	}
	return {};
}

nano::wallets::wallets (bool error_a, nano::node & node_a) :
	network_params{ node_a.config->network_params },
	kdf{ node_a.config->network_params.kdf_work },
	node (node_a),
	env (boost::polymorphic_downcast<nano::mdb_wallets_store *> (node_a.wallets_store_impl.get ())->environment),
	representatives{ node_a },
	rust_handle{ rsnano::rsn_lmdb_wallets_create (node_a.config->enable_voting, env.handle) },
	mutex{ rust_handle },
	wallet_actions{}
{
	{
		auto lock{ mutex.lock () };
		if (!error_a)
		{
			auto transaction (tx_begin_write ());
			auto store_l = dynamic_cast<nano::store::lmdb::component *> (&node.store);
			int status = !rsnano::rsn_lmdb_wallets_init (rust_handle, transaction->get_rust_handle ());

			auto wallet_ids{ get_wallet_ids (*transaction) };
			for (auto id : wallet_ids)
			{
				release_assert (items.find (id) == items.end ());
				std::string text;
				id.encode_hex (text);
				bool error = false;
				auto wallet (std::make_shared<nano::wallet> (error, *transaction, *this, text));
				if (!error)
				{
					items[id] = wallet;
				}
				else
				{
					// Couldn't open wallet
				}
			}
		}
		// Backup before upgrade wallets
		bool backup_required (false);
		if (node.config->backup_before_upgrade)
		{
			auto transaction (tx_begin_read ());
			for (auto & item : items)
			{
				if (item.second->store.version (*transaction) != nano::wallet_store::version_current)
				{
					backup_required = true;
					break;
				}
			}
		}
		if (backup_required)
		{
			rsnano::rsn_lmdb_store_create_backup_file (env.handle, nano::to_logger_handle (node_a.logger));
		}
		for (auto & item : items)
		{
			item.second->enter_initial_password ();
		}
	}
	if (node_a.config->enable_voting)
	{
		ongoing_compute_reps ();
	}
}

nano::wallets::~wallets ()
{
	wallet_actions.stop ();
	rsnano::rsn_lmdb_wallets_destroy (rust_handle);
}

size_t nano::wallets::wallet_count () const
{
	auto lock{ mutex.lock () };
	return items.size ();
}

size_t nano::wallets::representatives_count (nano::wallet_id const & id) const
{
	auto lock{ mutex.lock () };
	auto wallet = items.find (id);
	auto representatives_lk (wallet->second->representatives_mutex.lock ());
	return representatives_lk.size ();
}

nano::key_type nano::wallets::key_type (nano::wallet_id const & wallet_id, nano::raw_key const & key)
{
	auto lock{ mutex.lock () };
	auto wallet = items.find (wallet_id);

	if (wallet == items.end ())
	{
		return nano::key_type::unknown;
	}

	auto txn{ tx_begin_read () };
	nano::wallet_value wallet_val{ key, 0 };
	return wallet->second->store.key_type (wallet_val);
}

nano::wallets_error nano::wallets::get_representative (nano::wallet_id const & wallet_id, nano::account & representative)
{
	auto lock{ mutex.lock () };
	auto wallet = items.find (wallet_id);

	if (wallet == items.end ())
	{
		return nano::wallets_error::wallet_not_found;
	}

	auto txn{ tx_begin_read () };
	representative = wallet->second->store.representative (*txn);
	return nano::wallets_error::none;
}

nano::wallets_error nano::wallets::set_representative (nano::wallet_id const & wallet_id, nano::account const & rep, bool update_existing_accounts)
{
	std::vector<nano::account> accounts;
	{
		auto lock{ mutex.lock () };
		auto wallet = items.find (wallet_id);

		if (wallet == items.end ())
		{
			return nano::wallets_error::wallet_not_found;
		}

		{
			auto txn{ tx_begin_write () };
			if (update_existing_accounts && !wallet->second->store.valid_password (*txn))
			{
				return nano::wallets_error::wallet_locked;
			}

			wallet->second->store.representative_set (*txn, rep);
		}

		// Change representative for all wallet accounts
		if (update_existing_accounts)
		{
			auto txn{ tx_begin_read () };
			auto block_transaction (node.store.tx_begin_read ());
			for (auto i (wallet->second->store.begin (*txn)), n (wallet->second->store.end ()); i != n; ++i)
			{
				nano::account const & account (i->first);
				auto info = node.ledger.account_info (*block_transaction, account);
				if (info)
				{
					if (info->representative () != rep)
					{
						accounts.push_back (account);
					}
				}
			}
		}
	}

	for (auto & account : accounts)
	{
		(void)change_async (
		wallet_id, account, rep, [] (std::shared_ptr<nano::block> const &) {}, 0, false);
	}

	return nano::wallets_error::none;
}

nano::wallets_error nano::wallets::get_seed (nano::wallet_id const & wallet_id, nano::raw_key & prv_a) const
{
	auto lock{ mutex.lock () };
	auto wallet = items.find (wallet_id);

	if (wallet == items.end ())
	{
		return nano::wallets_error::wallet_not_found;
	}

	auto txn{ tx_begin_read () };
	if (!wallet->second->store.valid_password (*txn))
	{
		return nano::wallets_error::wallet_locked;
	}

	wallet->second->store.seed (prv_a, *txn);
	return nano::wallets_error::none;
}

nano::wallets_error nano::wallets::change_seed (nano::wallet_id const & wallet_id, nano::raw_key const & prv_a, uint32_t count, nano::public_key & first_account, uint32_t & restored_count)
{
	auto lock{ mutex.lock () };
	auto wallet = items.find (wallet_id);

	if (wallet == items.end ())
	{
		return nano::wallets_error::wallet_not_found;
	}

	auto txn{ tx_begin_write () };
	if (!wallet->second->store.valid_password (*txn))
	{
		return nano::wallets_error::wallet_locked;
	}

	first_account = wallet->second->change_seed (*txn, prv_a, count);
	restored_count = wallet->second->store.deterministic_index_get (*txn);
	return nano::wallets_error::none;
}

bool nano::wallets::ensure_wallet_is_unlocked (nano::wallet_id const & wallet_id, std::string const & password_a)
{
	auto lock{ mutex.lock () };
	auto existing{ items.find (wallet_id) };
	bool valid (false);
	{
		auto transaction{ tx_begin_write () };
		valid = existing->second->store.valid_password (*transaction);
		if (!valid)
		{
			valid = !existing->second->enter_password (*transaction, password_a);
		}
	}
	return valid;
}

bool nano::wallets::import_replace (nano::wallet_id const & wallet_id, std::string const & json_a, std::string const & password_a)
{
	auto lock{ mutex.lock () };
	auto existing{ items.find (wallet_id) };
	return existing->second->import (json_a, password_a);
}

bool nano::wallets::import (nano::wallet_id const & wallet_id, std::string const & json_a)
{
	auto lock{ mutex.lock () };
	auto txn (tx_begin_write ());
	bool error = true;
	nano::wallet wallet (error, *txn, node.wallets, wallet_id.to_string (), json_a);
	return error;
}

nano::wallets_error nano::wallets::decrypt (nano::wallet_id const & wallet_id, std::vector<std::pair<nano::account, nano::raw_key>> accounts) const
{
	auto lock{ mutex.lock () };
	auto wallet = items.find (wallet_id);

	if (wallet == items.end ())
	{
		return nano::wallets_error::wallet_not_found;
	}

	auto txn{ tx_begin_read () };
	if (!wallet->second->store.valid_password (*txn))
	{
		return nano::wallets_error::wallet_locked;
	}

	for (auto i (wallet->second->store.begin (*txn)), m (wallet->second->store.end ()); i != m; ++i)
	{
		nano::account const & account (i->first);
		nano::raw_key key;
		auto error (wallet->second->store.fetch (*txn, account, key));
		(void)error;
		debug_assert (!error);
		accounts.emplace_back (account, key);
	}
	return nano::wallets_error::none;
}

nano::wallets_error nano::wallets::fetch (nano::wallet_id const & wallet_id, nano::account const & pub, nano::raw_key & prv)
{
	auto lock{ mutex.lock () };
	auto wallet = items.find (wallet_id);

	if (wallet == items.end ())
	{
		return nano::wallets_error::wallet_not_found;
	}

	auto txn{ tx_begin_read () };
	if (!wallet->second->store.valid_password (*txn))
	{
		return nano::wallets_error::wallet_locked;
	}

	if (wallet->second->store.find (*txn, pub) == wallet->second->store.end ())
	{
		return nano::wallets_error::account_not_found;
	}

	if (wallet->second->store.fetch (*txn, pub, prv))
	{
		return nano::wallets_error::generic;
	}

	return nano::wallets_error::none;
}

std::vector<nano::wallet_id> nano::wallets::get_wallet_ids () const
{
	auto lock{ mutex.lock () };
	std::vector<nano::wallet_id> result{};
	result.reserve (items.size ());
	for (auto i (items.begin ()), n (items.end ()); i != n; ++i)
	{
		result.push_back (i->first);
	}
	return result;
}

nano::wallets_error nano::wallets::get_accounts (nano::wallet_id const & wallet_id, std::vector<nano::account> & accounts)
{
	auto lock{ mutex.lock () };
	auto wallet = items.find (wallet_id);

	if (wallet == items.end ())
	{
		return nano::wallets_error::wallet_not_found;
	}
	auto txn (tx_begin_read ());

	for (auto j (wallet->second->store.begin (*txn)), m (wallet->second->store.end ()); j != m; ++j)
	{
		accounts.push_back (nano::account (j->first));
	}

	return nano::wallets_error::none;
}

std::vector<nano::account> nano::wallets::get_accounts (size_t max_results)
{
	auto lock{ mutex.lock () };
	auto const transaction (tx_begin_read ());
	std::vector<nano::account> accounts;
	for (auto i (items.begin ()), n (items.end ()); i != n && accounts.size () < max_results; ++i)
	{
		auto & wallet (*i->second);
		for (auto j (wallet.store.begin (*transaction)), m (wallet.store.end ()); j != m && accounts.size () < max_results; ++j)
		{
			nano::account account (j->first);
			accounts.push_back (account);
		}
	}
	return accounts;
}

nano::wallets_error nano::wallets::work_get (nano::wallet_id const & wallet_id, nano::account const & account, uint64_t & work)
{
	auto lock{ mutex.lock () };
	auto wallet (items.find (wallet_id));
	if (wallet == items.end ())
	{
		return nano::wallets_error::wallet_not_found;
	}
	auto txn{ tx_begin_write () };
	if (wallet->second->store.find (*txn, account) == wallet->second->store.end ())
	{
		return nano::wallets_error::account_not_found;
	}
	wallet->second->store.work_get (*txn, account, work);
	return nano::wallets_error::none;
}

uint64_t nano::wallets::work_get (nano::wallet_id const & wallet_id, nano::account const & account)
{
	auto lock{ mutex.lock () };
	auto transaction (tx_begin_read ());
	auto wallet{ items.find (wallet_id) };
	uint64_t work (1);
	wallet->second->store.work_get (*transaction, account, work);
	return work;
}

nano::wallets_error nano::wallets::work_set (nano::wallet_id const & wallet_id, nano::account const & account, uint64_t work)
{
	auto lock{ mutex.lock () };
	auto wallet (items.find (wallet_id));
	if (wallet == items.end ())
	{
		return nano::wallets_error::wallet_not_found;
	}
	auto txn{ tx_begin_write () };
	if (wallet->second->store.find (*txn, account) == wallet->second->store.end ())
	{
		return nano::wallets_error::account_not_found;
	}

	wallet->second->store.work_put (*txn, account, work);
	return nano::wallets_error::none;
}

nano::wallets_error nano::wallets::remove_account (nano::wallet_id const & wallet_id, nano::account const & account_id)
{
	auto lock{ mutex.lock () };
	auto wallet (items.find (wallet_id));
	if (wallet == items.end ())
	{
		return nano::wallets_error::wallet_not_found;
	}
	auto txn{ tx_begin_write () };
	if (!wallet->second->store.valid_password (*txn))
	{
		return nano::wallets_error::wallet_locked;
	}
	if (wallet->second->store.find (*txn, account_id) == wallet->second->store.end ())
	{
		return nano::wallets_error::account_not_found;
	}
	wallet->second->store.erase (*txn, account_id);
	return nano::wallets_error::none;
}

bool nano::wallets::move_accounts (nano::wallet_id const & source_id, nano::wallet_id const & target_id, std::vector<nano::public_key> const & accounts)
{
	auto lock{ mutex.lock () };
	auto existing (items.find (source_id));
	auto source (existing->second);
	auto transaction (tx_begin_write ());
	auto target{ items.find (target_id) };
	auto error (target->second->store.move (*transaction, source->store, accounts));
	return error;
}

bool nano::wallets::wallet_exists (nano::wallet_id const & id) const
{
	auto lock{ mutex.lock () };
	return items.find (id) != items.end ();
}

nano::wallet_id nano::wallets::first_wallet_id () const
{
	auto lock{ mutex.lock () };
	return items.begin ()->first;
}

nano::wallets_error nano::wallets::insert_adhoc (nano::wallet_id const & wallet_id, nano::raw_key const & key_a, bool generate_work_a)
{
	auto lock{ mutex.lock () };
	auto wallet (items.find (wallet_id));
	if (wallet == items.end ())
	{
		return nano::wallets_error::wallet_not_found;
	}
	auto txn{ tx_begin_read () };
	if (!wallet->second->store.valid_password (*txn))
	{
		return nano::wallets_error::wallet_locked;
	}

	txn->reset ();
	wallet->second->insert_adhoc (key_a, generate_work_a);
	return nano::wallets_error::none;
}

nano::wallets_error nano::wallets::insert_adhoc (nano::wallet_id const & wallet_id, nano::raw_key const & key_a, bool generate_work_a, nano::public_key & account)
{
	auto lock{ mutex.lock () };
	auto wallet (items.find (wallet_id));
	if (wallet == items.end ())
	{
		return nano::wallets_error::wallet_not_found;
	}
	auto txn{ tx_begin_read () };
	if (!wallet->second->store.valid_password (*txn))
	{
		return nano::wallets_error::wallet_locked;
	}
	txn->reset ();
	account = wallet->second->insert_adhoc (key_a, generate_work_a);
	return nano::wallets_error::none;
}

nano::wallets_error nano::wallets::insert_watch (nano::wallet_id const & wallet_id, std::vector<nano::public_key> const & accounts)
{
	auto lock{ mutex.lock () };
	auto wallet (items.find (wallet_id));
	if (wallet == items.end ())
	{
		return nano::wallets_error::wallet_not_found;
	}
	auto txn{ tx_begin_write () };
	if (!wallet->second->store.valid_password (*txn))
	{
		return nano::wallets_error::wallet_locked;
	}

	for (auto & account : accounts)
	{
		if (wallet->second->insert_watch (*txn, account))
		{
			return nano::wallets_error::bad_public_key;
		}
	}

	return nano::wallets_error::none;
}

void nano::wallets::set_password (nano::wallet_id const & wallet_id, nano::raw_key const & password)
{
	auto lock{ mutex.lock () };
	auto wallet{ items.find (wallet_id) };
	wallet->second->store.set_password (password);
}

void nano::wallets::password (nano::wallet_id const & wallet_id, nano::raw_key & password_a) const
{
	auto lock{ mutex.lock () };
	auto wallet{ items.find (wallet_id) };
	wallet->second->store.password (password_a);
}

nano::wallets_error nano::wallets::enter_password (nano::wallet_id const & wallet_id, std::string const & password_a)
{
	auto lock{ mutex.lock () };
	auto wallet (items.find (wallet_id));
	if (wallet == items.end ())
	{
		return nano::wallets_error::wallet_not_found;
	}
	auto txn{ tx_begin_write () };

	bool error = wallet->second->enter_password (*txn, password_a);
	if (error)
	{
		return nano::wallets_error::invalid_password;
	}
	return nano::wallets_error::none;
}

void nano::wallets::enter_initial_password (nano::wallet_id const & wallet_id)
{
	auto lock{ mutex.lock () };
	auto wallet{ items.find (wallet_id) };
	wallet->second->enter_initial_password ();
}

nano::wallets_error nano::wallets::valid_password (nano::wallet_id const & wallet_id, bool & valid)
{
	auto wallet (items.find (wallet_id));
	if (wallet == items.end ())
	{
		return nano::wallets_error::wallet_not_found;
	}
	auto txn{ tx_begin_read () };
	valid = wallet->second->store.valid_password (*txn);
	return nano::wallets_error::none;
}

nano::wallets_error nano::wallets::attempt_password (nano::wallet_id const & wallet_id, std::string const & password, bool & error)
{
	auto lock{ mutex.lock () };
	auto wallet (items.find (wallet_id));
	if (wallet == items.end ())
	{
		return nano::wallets_error::wallet_not_found;
	}
	auto txn{ tx_begin_write () };
	error = wallet->second->store.attempt_password (*txn, password);
	return nano::wallets_error::none;
}

nano::wallets_error nano::wallets::rekey (nano::wallet_id const wallet_id, std::string const & password)
{
	auto lock{ mutex.lock () };
	auto wallet (items.find (wallet_id));
	if (wallet == items.end ())
	{
		return nano::wallets_error::wallet_not_found;
	}
	auto txn{ tx_begin_write () };
	if (!wallet->second->store.valid_password (*txn))
	{
		return nano::wallets_error::wallet_locked;
	}

	if (wallet->second->store.rekey (*txn, password))
	{
		return nano::wallets_error::generic;
	}
	return nano::wallets_error::none;
}

nano::wallets_error nano::wallets::lock (nano::wallet_id const & wallet_id)
{
	auto lock{ mutex.lock () };
	auto wallet (items.find (wallet_id));
	if (wallet == items.end ())
	{
		return nano::wallets_error::wallet_not_found;
	}
	wallet->second->store.lock ();
	return nano::wallets_error::none;
}

nano::wallets_error nano::wallets::deterministic_insert (nano::wallet_id const & wallet_id, uint32_t const index, bool generate_work_a, nano::account & account)
{
	auto lock{ mutex.lock () };
	auto wallet (items.find (wallet_id));
	if (wallet == items.end ())
	{
		return nano::wallets_error::wallet_not_found;
	}
	auto txn{ tx_begin_write () };
	if (!wallet->second->store.valid_password (*txn))
	{
		return nano::wallets_error::wallet_locked;
	}

	account = wallet->second->store.deterministic_insert (*txn, index);
	if (generate_work_a)
	{
		wallet->second->work_ensure (account, account);
	}
	return nano::wallets_error::none;
}

nano::wallets_error nano::wallets::deterministic_insert (nano::wallet_id const & wallet_id, bool generate_work_a, nano::account & account)
{
	auto lock{ mutex.lock () };
	auto wallet (items.find (wallet_id));
	if (wallet == items.end ())
	{
		return nano::wallets_error::wallet_not_found;
	}
	auto txn{ tx_begin_write () };
	if (!wallet->second->store.valid_password (*txn))
	{
		return nano::wallets_error::wallet_locked;
	}

	account = wallet->second->deterministic_insert (*txn, generate_work_a);
	return nano::wallets_error::none;
}

nano::wallets_error nano::wallets::deterministic_index_get (nano::wallet_id const & wallet_id, uint32_t & index)
{
	index = 0;
	auto lock{ mutex.lock () };
	auto wallet (items.find (wallet_id));
	if (wallet == items.end ())
	{
		return nano::wallets_error::wallet_not_found;
	}
	auto txn{ tx_begin_read () };
	index = wallet->second->store.deterministic_index_get (*txn);
	return nano::wallets_error::none;
}

void nano::wallets::backup (std::filesystem::path const & backup_path)
{
	auto lock{ mutex.lock () };
	auto transaction{ tx_begin_read () };
	for (auto i (items.begin ()), n (items.end ()); i != n; ++i)
	{
		boost::system::error_code error_chmod;

		std::filesystem::create_directories (backup_path);
		nano::set_secure_perm_directory (backup_path, error_chmod);
		i->second->store.write_backup (*transaction, backup_path / (i->first.to_string () + ".json"));
	}
}

void nano::wallets::work_cache_blocking (nano::wallet_id const & wallet_id, nano::account const & account_a, nano::root const & root_a)
{
	auto lock{ mutex.lock () };
	auto wallet{ items.find (wallet_id) };
	wallet->second->work_cache_blocking (account_a, root_a);
}

std::shared_ptr<nano::block> nano::wallets::send_action (nano::wallet_id const & wallet_id, nano::account const & source_a, nano::account const & account_a, nano::uint128_t const & amount_a, uint64_t work_a, bool generate_work_a, boost::optional<std::string> id_a)
{
	auto lock{ mutex.lock () };
	auto wallet = items.find (wallet_id);
	return wallet->second->send_action (source_a, account_a, amount_a, work_a, generate_work_a, id_a);
}

std::shared_ptr<nano::block> nano::wallets::receive_action (nano::wallet_id const & wallet_id, nano::block_hash const & send_hash_a, nano::account const & representative_a, nano::uint128_union const & amount_a, nano::account const & account_a, uint64_t work_a, bool generate_work_a)
{
	auto lock{ mutex.lock () };
	auto wallet = items.find (wallet_id);
	return wallet->second->receive_action (send_hash_a, representative_a, amount_a, account_a, work_a, generate_work_a);
}

std::shared_ptr<nano::block> nano::wallets::change_action (nano::wallet_id const & wallet_id, nano::account const & source_a, nano::account const & representative_a, uint64_t work_a, bool generate_work_a)
{
	auto lock{ mutex.lock () };
	auto wallet = items.find (wallet_id);
	return wallet->second->change_action (source_a, representative_a, work_a, generate_work_a);
}

nano::block_hash nano::wallets::send_sync (nano::wallet_id const & wallet_id, nano::account const & source_a, nano::account const & account_a, nano::uint128_t const & amount_a)
{
	auto lock{ mutex.lock () };
	auto wallet = items.find (wallet_id);

	std::promise<nano::block_hash> result;
	std::future<nano::block_hash> future = result.get_future ();
	wallet->second->send_async (
	source_a, account_a, amount_a, [&result] (std::shared_ptr<nano::block> const & block_a) {
		result.set_value (block_a->hash ());
	},
	true);
	return future.get ();
}

bool nano::wallets::receive_sync (nano::wallet_id const & wallet_id, std::shared_ptr<nano::block> const & block_a, nano::account const & representative_a, nano::uint128_t const & amount_a)
{
	auto lock{ mutex.lock () };
	auto wallet = items.find (wallet_id);
	return wallet->second->receive_sync (block_a, representative_a, amount_a);
}

bool nano::wallets::change_sync (nano::wallet_id const & wallet_id, nano::account const & source_a, nano::account const & representative_a)
{
	auto lock{ mutex.lock () };
	auto wallet = items.find (wallet_id);
	return wallet->second->change_sync (source_a, representative_a);
}

nano::wallets_error nano::wallets::receive_async (nano::wallet_id const & wallet_id, nano::block_hash const & hash_a, nano::account const & representative_a, nano::uint128_t const & amount_a, nano::account const & account_a, std::function<void (std::shared_ptr<nano::block> const &)> const & action_a, uint64_t work_a, bool generate_work_a)
{
	auto lock{ mutex.lock () };
	auto wallet (items.find (wallet_id));
	if (wallet == items.end ())
	{
		return nano::wallets_error::wallet_not_found;
	}
	auto txn{ tx_begin_write () };
	if (!wallet->second->store.valid_password (*txn))
	{
		return nano::wallets_error::wallet_locked;
	}
	if (wallet->second->store.find (*txn, account_a) == wallet->second->store.end ())
	{
		return nano::wallets_error::account_not_found;
	}

	wallet->second->receive_async (hash_a, representative_a, amount_a, account_a, action_a, work_a, generate_work_a);
	return nano::wallets_error::none;
}

nano::wallets_error nano::wallets::change_async (nano::wallet_id const & wallet_id, nano::account const & source_a, nano::account const & representative_a, std::function<void (std::shared_ptr<nano::block> const &)> const & action_a, uint64_t work_a, bool generate_work_a)
{
	auto lock{ mutex.lock () };
	auto wallet (items.find (wallet_id));
	if (wallet == items.end ())
	{
		return nano::wallets_error::wallet_not_found;
	}
	auto txn{ tx_begin_write () };
	if (!wallet->second->store.valid_password (*txn))
	{
		return nano::wallets_error::wallet_locked;
	}
	if (wallet->second->store.find (*txn, source_a) == wallet->second->store.end ())
	{
		return nano::wallets_error::account_not_found;
	}
	wallet->second->change_async (source_a, representative_a, action_a, generate_work_a);
	return nano::wallets_error::none;
}

nano::wallets_error nano::wallets::send_async (nano::wallet_id const & wallet_id, nano::account const & source_a, nano::account const & account_a, nano::uint128_t const & amount_a, std::function<void (std::shared_ptr<nano::block> const &)> const & action_a, uint64_t work_a, bool generate_work_a, boost::optional<std::string> id_a)
{
	auto lock{ mutex.lock () };
	auto wallet (items.find (wallet_id));
	if (wallet == items.end ())
	{
		return nano::wallets_error::wallet_not_found;
	}
	auto txn{ tx_begin_write () };
	if (!wallet->second->store.valid_password (*txn))
	{
		return nano::wallets_error::wallet_locked;
	}
	if (wallet->second->store.find (*txn, source_a) == wallet->second->store.end ())
	{
		return nano::wallets_error::account_not_found;
	}

	wallet->second->send_async (source_a, account_a, amount_a, action_a, work_a, generate_work_a, id_a);
	return nano::wallets_error::none;
}

void nano::wallets::receive_confirmed (store::transaction const & block_transaction_a, nano::block_hash const & hash_a, nano::account const & destination_a)
{
	std::unordered_map<nano::wallet_id, std::shared_ptr<nano::wallet>> wallets_l;
	std::unique_ptr<nano::store::read_transaction> wallet_transaction;
	{
		auto lk{ mutex.lock () };
		wallets_l = items;
		wallet_transaction = tx_begin_read ();
	}
	for ([[maybe_unused]] auto const & [id, wallet] : wallets_l)
	{
		if (wallet->store.exists (*wallet_transaction, destination_a))
		{
			nano::account representative;
			representative = wallet->store.representative (*wallet_transaction);
			auto pending = node.ledger.pending_info (block_transaction_a, nano::pending_key (destination_a, hash_a));
			if (pending)
			{
				auto amount (pending->amount.number ());
				wallet->receive_async (hash_a, representative, amount, destination_a, [] (std::shared_ptr<nano::block> const &) {});
			}
			else
			{
				if (!node.ledger.block_or_pruned_exists (block_transaction_a, hash_a))
				{
					node.logger->try_log (boost::str (boost::format ("Confirmed block is missing:  %1%") % hash_a.to_string ()));
					debug_assert (false && "Confirmed block is missing");
				}
				else
				{
					node.logger->try_log (boost::str (boost::format ("Block %1% has already been received") % hash_a.to_string ()));
				}
			}
		}
	}
}

nano::wallets_error nano::wallets::serialize (nano::wallet_id const & wallet_id, std::string & json)
{
	auto wallet (items.find (wallet_id));
	if (wallet == items.end ())
	{
		return nano::wallets_error::wallet_not_found;
	}
	auto txn{ tx_begin_read () };
	wallet->second->serialize (json);
	return nano::wallets_error::none;
}

void nano::wallets::create (nano::wallet_id const & id_a)
{
	auto lock{ mutex.lock () };
	debug_assert (items.find (id_a) == items.end ());
	std::shared_ptr<nano::wallet> result;
	bool error;
	{
		auto transaction (tx_begin_write ());
		result = std::make_shared<nano::wallet> (error, *transaction, *this, id_a.to_string ());
	}
	if (!error)
	{
		items[id_a] = result;
		result->enter_initial_password ();
	}
}

nano::wallets_error nano::wallets::search_receivable (nano::wallet_id const & wallet_id)
{
	auto lock{ mutex.lock () };
	auto wallet (items.find (wallet_id));
	if (wallet == items.end ())
	{
		return nano::wallets_error::wallet_not_found;
	}
	auto txn{ tx_begin_read () };
	if (!wallet->second->store.valid_password (*txn))
	{
		return nano::wallets_error::wallet_locked;
	}

	wallet->second->search_receivable (*txn);
	return nano::wallets_error::none;
}

void nano::wallets::search_receivable_all ()
{
	std::unordered_map<nano::wallet_id, std::shared_ptr<nano::wallet>> wallets_l;
	{
		auto lk{ mutex.lock () };
		wallets_l = items;
	}
	auto wallet_transaction (tx_begin_read ());
	for (auto const & [id, wallet] : wallets_l)
	{
		wallet->search_receivable (*wallet_transaction);
	}
}

void nano::wallets::destroy (nano::wallet_id const & id_a)
{
	auto lock{ mutex.lock () };
	auto transaction (tx_begin_write ());
	// action_mutex should be locked after transactions to prevent deadlocks in deterministic_insert () & insert_adhoc ()
	auto action_lock{ wallet_actions.lock () };
	auto existing (items.find (id_a));
	debug_assert (existing != items.end ());
	auto wallet (existing->second);
	items.erase (existing);
	wallet->store.destroy (*transaction);
}

void nano::wallets::reload ()
{
	auto lock{ mutex.lock () };
	auto transaction (tx_begin_write ());
	std::unordered_set<nano::uint256_union> stored_items;
	auto wallet_ids{ get_wallet_ids (*transaction) };
	for (auto id : wallet_ids)
	{
		// New wallet
		if (items.find (id) == items.end ())
		{
			bool error = false;
			std::string text;
			id.encode_hex (text);
			auto wallet (std::make_shared<nano::wallet> (error, *transaction, *this, text));
			if (!error)
			{
				items[id] = wallet;
			}
		}
		// List of wallets on disk
		stored_items.insert (id);
	}
	// Delete non existing wallets from memory
	std::vector<nano::wallet_id> deleted_items;
	for (auto i : items)
	{
		if (stored_items.find (i.first) == stored_items.end ())
		{
			deleted_items.push_back (i.first);
		}
	}
	for (auto & i : deleted_items)
	{
		debug_assert (items.find (i) == items.end ());
		items.erase (i);
	}
}

void nano::wallets::foreach_representative (std::function<void (nano::public_key const & pub_a, nano::raw_key const & prv_a)> const & action_a)
{
	if (node.config->enable_voting)
	{
		std::vector<std::pair<nano::public_key const, nano::raw_key const>> action_accounts_l;
		{
			auto transaction_l (tx_begin_read ());
			auto lock{ mutex.lock () };
			for (auto i (items.begin ()), n (items.end ()); i != n; ++i)
			{
				auto & wallet (*i->second);
				std::unordered_set<nano::account> representatives_l;
				{
					auto representatives_lock{ wallet.representatives_mutex.lock () };
					representatives_l = representatives_lock.get_all ();
				}
				for (auto const & account : representatives_l)
				{
					if (wallet.store.exists (*transaction_l, account))
					{
						if (!node.ledger.weight (account).is_zero ())
						{
							if (wallet.store.valid_password (*transaction_l))
							{
								nano::raw_key prv;
								auto error (wallet.store.fetch (*transaction_l, account, prv));
								(void)error;
								debug_assert (!error);
								action_accounts_l.emplace_back (account, prv);
							}
							else
							{
								static auto last_log = std::chrono::steady_clock::time_point ();
								if (last_log < std::chrono::steady_clock::now () - std::chrono::seconds (60))
								{
									last_log = std::chrono::steady_clock::now ();
									node.logger->always_log (boost::str (boost::format ("Representative locked inside wallet %1%") % i->first.to_string ()));
								}
							}
						}
					}
				}
			}
		}
		for (auto const & representative : action_accounts_l)
		{
			action_a (representative.first, representative.second);
		}
	}
}

bool nano::wallets::exists (nano::account const & account_a)
{
	auto lock{ mutex.lock () };
	auto txn{ tx_begin_read () };
	auto result (false);
	for (auto i (items.begin ()), n (items.end ()); !result && i != n; ++i)
	{
		result = i->second->store.exists (*txn, account_a);
	}
	return result;
}

std::unique_ptr<nano::store::write_transaction> nano::wallets::tx_begin_write ()
{
	return env.tx_begin_write ();
}

std::unique_ptr<nano::store::read_transaction> nano::wallets::tx_begin_read () const
{
	return env.tx_begin_read ();
}

void nano::wallets::clear_send_ids ()
{
	auto txn{ env.tx_begin_write () };
	rsnano::rsn_lmdb_wallets_clear_send_ids (rust_handle, txn->get_rust_handle ());
}

size_t nano::wallets::voting_reps_count () const
{
	nano::lock_guard<nano::mutex> counts_guard{ representatives.reps_cache_mutex };
	return representatives.voting;
}

bool nano::wallets::have_half_rep () const
{
	nano::lock_guard<nano::mutex> counts_guard{ representatives.reps_cache_mutex };
	return representatives.have_half_rep ();
}

bool nano::wallets::rep_exists (nano::account const & rep) const
{
	nano::lock_guard<nano::mutex> counts_guard{ representatives.reps_cache_mutex };
	return representatives.exists (rep);
}

bool nano::wallets::should_republish_vote (nano::account const & voting_account) const
{
	nano::lock_guard<nano::mutex> counts_guard{ representatives.reps_cache_mutex };
	return !representatives.have_half_rep () && !representatives.exists (voting_account);
}

void nano::wallets::compute_reps ()
{
	auto guard{ mutex.lock () };
	nano::lock_guard<nano::mutex> counts_guard{ representatives.reps_cache_mutex };
	representatives.clear ();
	auto half_principal_weight (node.minimum_principal_weight () / 2);
	auto transaction (tx_begin_read ());
	for (auto i (items.begin ()), n (items.end ()); i != n; ++i)
	{
		auto & wallet (*i->second);
		std::unordered_set<nano::account> representatives_l;
		for (auto ii (wallet.store.begin (*transaction)), nn (wallet.store.end ()); ii != nn; ++ii)
		{
			auto account (ii->first);
			if (representatives.check_rep (account, half_principal_weight, false))
			{
				representatives_l.insert (account);
			}
		}
		auto representatives_guard{ wallet.representatives_mutex.lock () };
		representatives_guard.set (representatives_l);
	}
}

void nano::wallets::ongoing_compute_reps ()
{
	compute_reps ();
	auto & node_l (node);
	// Representation drifts quickly on the test network but very slowly on the live network
	auto compute_delay = network_params.network.is_dev_network () ? std::chrono::milliseconds (10) : (network_params.network.is_test_network () ? std::chrono::milliseconds (nano::test_scan_wallet_reps_delay ()) : std::chrono::minutes (15));
	node.workers->add_timed_task (std::chrono::steady_clock::now () + compute_delay, [&node_l] () {
		node_l.wallets.ongoing_compute_reps ();
	});
}

std::vector<nano::wallet_id> nano::wallets::get_wallet_ids (nano::store::transaction const & transaction_a)
{
	rsnano::U256ArrayDto dto;
	rsnano::rsn_lmdb_wallets_get_wallet_ids (rust_handle, transaction_a.get_rust_handle (), &dto);
	std::vector<nano::wallet_id> wallet_ids;
	for (int i = 0; i < dto.count; ++i)
	{
		nano::wallet_id id;
		std::copy (std::begin (dto.items[i]), std::end (dto.items[i]), std::begin (id.bytes));
		wallet_ids.push_back (id);
	}
	rsnano::rsn_u256_array_destroy (&dto);
	return wallet_ids;
}

nano::block_hash nano::wallets::get_block_hash (bool & error_a, nano::store::transaction const & transaction_a, std::string const & id_a)
{
	nano::block_hash result;
	error_a = !rsnano::rsn_lmdb_wallets_get_block_hash (rust_handle, transaction_a.get_rust_handle (), id_a.c_str (), result.bytes.data ());
	return result;
}

bool nano::wallets::set_block_hash (nano::store::transaction const & transaction_a, std::string const & id_a, nano::block_hash const & hash)
{
	return !rsnano::rsn_lmdb_wallets_set_block_hash (rust_handle, transaction_a.get_rust_handle (), id_a.c_str (), hash.bytes.data ());
}

nano::uint128_t const nano::wallets::generate_priority = std::numeric_limits<nano::uint128_t>::max ();
nano::uint128_t const nano::wallets::high_priority = std::numeric_limits<nano::uint128_t>::max () - 1;

namespace
{
nano::store::iterator<nano::account, nano::wallet_value> to_iterator (rsnano::LmdbIteratorHandle * it_handle)
{
	if (it_handle == nullptr)
	{
		return { nullptr };
	}

	return { std::make_unique<nano::store::lmdb::iterator<nano::account, nano::wallet_value>> (it_handle) };
}
}

nano::store::iterator<nano::account, nano::wallet_value> nano::wallet_store::begin (nano::store::transaction const & transaction_a)
{
	auto it_handle{ rsnano::rsn_lmdb_wallet_store_begin (rust_handle, transaction_a.get_rust_handle ()) };
	return to_iterator (it_handle);
}

nano::store::iterator<nano::account, nano::wallet_value> nano::wallet_store::begin (store::transaction const & transaction_a, nano::account const & key)
{
	auto it_handle{ rsnano::rsn_lmdb_wallet_store_begin_at_account (rust_handle, transaction_a.get_rust_handle (), key.bytes.data ()) };
	return to_iterator (it_handle);
}

nano::store::iterator<nano::account, nano::wallet_value> nano::wallet_store::find (store::transaction const & transaction_a, nano::account const & key)
{
	auto it_handle = rsnano::rsn_lmdb_wallet_store_find (rust_handle, transaction_a.get_rust_handle (), key.bytes.data ());
	return to_iterator (it_handle);
}

nano::store::iterator<nano::account, nano::wallet_value> nano::wallet_store::end ()
{
	return { nullptr };
}
nano::mdb_wallets_store::mdb_wallets_store (std::filesystem::path const & path_a, nano::lmdb_config const & lmdb_config_a) :
	environment (error, path_a, nano::store::lmdb::env::options::make ().set_config (lmdb_config_a).override_config_sync (nano::lmdb_config::sync_strategy::always).override_config_map_size (1ULL * 1024 * 1024 * 1024))
{
}

bool nano::mdb_wallets_store::init_error () const
{
	return error;
}

std::unique_ptr<nano::container_info_component> nano::collect_container_info (wallets & wallets, std::string const & name)
{
	std::size_t items_count;
	std::size_t actions_count = wallets.wallet_actions.size ();
	{
		auto guard{ wallets.mutex.lock () };
		items_count = wallets.items.size ();
	}

	auto sizeof_item_element = sizeof (decltype (wallets.items)::value_type);
	auto sizeof_actions_element = sizeof (uintptr_t) * 2;
	auto composite = std::make_unique<container_info_composite> (name);
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "items", items_count, sizeof_item_element }));
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "actions", actions_count, sizeof_actions_element }));
	return composite;
}
