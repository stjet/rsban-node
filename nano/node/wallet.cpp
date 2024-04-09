#include "nano/store/lmdb/lmdb_env.hpp"

#include <nano/crypto_lib/random_pool.hpp>
#include <nano/lib/blocks.hpp>
#include <nano/lib/rsnano.hpp>
#include <nano/lib/rsnanoutils.hpp>
#include <nano/lib/threading.hpp>
#include <nano/lib/utility.hpp>
#include <nano/node/election.hpp>
#include <nano/node/node.hpp>
#include <nano/node/wallet.hpp>
#include <nano/secure/ledger.hpp>
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
	rust_handle{ rsnano::rsn_lmdb_wallet_store_create2 (fanout_a, kdf_a.handle, transaction_a.get_rust_handle (), wallet_a.c_str (), json_a.c_str ()) }
{
	init_a = rust_handle == nullptr;
}

nano::wallet_store::wallet_store (bool & init_a, nano::kdf & kdf_a, store::transaction & transaction_a, nano::account representative_a, unsigned fanout_a, std::string const & wallet_a) :
	rust_handle{ rsnano::rsn_lmdb_wallet_store_create (fanout_a, kdf_a.handle, transaction_a.get_rust_handle (), representative_a.bytes.data (), wallet_a.c_str ()) }
{
	init_a = rust_handle == nullptr;
}

nano::wallet_store::wallet_store (rsnano::LmdbWalletStoreHandle * handle) :
	rust_handle{ handle }
{
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

nano::key_type nano::wallet_store::key_type (store::transaction const & transaction_a, nano::account const & account)
{
	return static_cast<nano::key_type> (rsnano::rsn_lmdb_wallet_store_key_type (rust_handle, transaction_a.get_rust_handle (), account.bytes.data ()));
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

void nano::wallet_store::destroy (store::transaction const & transaction_a)
{
	if (rust_handle != nullptr)
		rsnano::rsn_lmdb_wallet_store_destroy2 (rust_handle, transaction_a.get_rust_handle ());
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
	handle{ rsnano::rsn_wallet_action_thread_create () }
{
}

nano::wallet_action_thread::~wallet_action_thread ()
{
	rsnano::rsn_wallet_action_thread_destroy (handle);
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
	rsnano::rsn_wallet_action_thread_stop (handle);
	if (thread.joinable ())
	{
		thread.join ();
	}
}

namespace
{
void wrapped_wallet_action_callback (void * context, rsnano::WalletHandle * wallet_handle)
{
	auto action = static_cast<std::function<void (nano::wallet &)> *> (context);
	auto wallet{ std::make_shared<nano::wallet> (wallet_handle) };
	(*action) (*wallet);
}

void delete_wallet_action_context (void * context)
{
	auto action = static_cast<std::function<void (nano::wallet &)> *> (context);
	delete action;
}
}

void nano::wallet_action_thread::queue_wallet_action (nano::uint128_t const & amount_a, std::shared_ptr<nano::wallet> const & wallet_a, std::function<void (nano::wallet &)> action_a)
{
	nano::amount amount{ amount_a };
	auto context = new std::function<void (nano::wallet &)> (action_a);
	rsnano::rsn_wallet_action_thread_queue_wallet_action (
	handle,
	amount.bytes.data (),
	wallet_a->handle,
	wrapped_wallet_action_callback,
	context,
	delete_wallet_action_context);
}

nano::wallet_action_thread::actions_lock nano::wallet_action_thread::lock ()
{
	return nano::wallet_action_thread::actions_lock{ rsnano::rsn_wallet_action_lock (handle) };
}

size_t nano::wallet_action_thread::size ()
{
	return rsnano::rsn_wallet_action_thread_len (handle);
}

namespace
{
void wrapped_wallet_action_observer (void * context, bool active)
{
	auto callback = static_cast<std::function<void (bool)> *> (context);
	(*callback) (active);
}

void delete_wallet_action_observer_context (void * context)
{
	auto callback = static_cast<std::function<void (bool)> *> (context);
	delete callback;
}
}

void nano::wallet_action_thread::set_observer (std::function<void (bool)> observer_a)
{
	auto context = new std::function<void (bool)> (observer_a);
	rsnano::rsn_wallet_action_thread_set_observer (handle, wrapped_wallet_action_observer, context, delete_wallet_action_observer_context);
}

void nano::wallet_action_thread::do_wallet_actions ()
{
	rsnano::rsn_wallet_action_thread_do_wallet_actions (handle);
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

namespace
{
rsnano::WalletHandle * create_wallet_handle (
nano::node & node,
nano::wallets & wallets_a,
nano::store::transaction & transaction_a,
nano::account representative,
std::string const & wallet_path,
const char * json)
{
	return rsnano::rsn_wallet_create (
	node.ledger.handle,
	&node.network_params.work.dto,
	node.config->password_fanout,
	wallets_a.kdf.handle,
	transaction_a.get_rust_handle (),
	representative.bytes.data (),
	wallet_path.c_str (),
	json);
}
}

nano::wallet::wallet (bool & init_a, store::transaction & transaction_a, nano::wallets & wallets_a, std::string const & wallet_a) :
	handle{ create_wallet_handle (wallets_a.node, wallets_a, transaction_a, wallets_a.node.config->random_representative (), wallet_a, nullptr) },
	store{ rsnano::rsn_wallet_store (handle) },
	representatives_mutex{ handle }
{
	init_a = handle == nullptr;
}

nano::wallet::wallet (bool & init_a, store::transaction & transaction_a, nano::wallets & wallets_a, std::string const & wallet_a, std::string const & json) :
	handle{ create_wallet_handle (wallets_a.node, wallets_a, transaction_a, wallets_a.node.config->random_representative (), wallet_a, json.c_str ()) },
	store{ rsnano::rsn_wallet_store (handle) },
	representatives_mutex{ handle }
{
	init_a = handle == nullptr;
}

nano::wallet::wallet (rsnano::WalletHandle * handle) :
	store{ rsnano::rsn_wallet_store (handle) },
	handle{ handle },
	representatives_mutex{ handle }
{
}

nano::wallet::~wallet ()
{
	if (handle != nullptr)
		rsnano::rsn_wallet_destroy (handle);
}

bool nano::wallet::insert_watch (store::transaction const & transaction_a, nano::public_key const & pub_a)
{
	return store.insert_watch (transaction_a, pub_a);
}

// Update work for account if latest root is root_a
void nano::wallet::work_update (store::transaction const & transaction_a, nano::account const & account_a, nano::root const & root_a, uint64_t work_a)
{
	rsnano::rsn_wallet_work_update (handle, transaction_a.get_rust_handle (), account_a.bytes.data (), root_a.bytes.data (), work_a);
}

uint32_t nano::wallet::deterministic_check (store::transaction const & transaction_a, uint32_t index)
{
	return rsnano::rsn_wallet_deterministic_check (handle, transaction_a.get_rust_handle (), index);
}

bool nano::wallet::live ()
{
	return rsnano::rsn_wallet_live (handle);
}

size_t nano::wallet::representatives_count () const
{
	auto representatives_lk (representatives_mutex.lock ());
	return representatives_lk.size ();
}

void nano::wallet::insert_representative (nano::account const & rep)
{
	auto representatives_lk (representatives_mutex.lock ());
	representatives_lk.insert (rep);
}

std::unordered_set<nano::account> nano::wallet::get_representatives ()
{
	auto representatives_lk (representatives_mutex.lock ());
	return representatives_lk.get_all ();
}

void nano::wallet::set_representatives (std::unordered_set<nano::account> const & reps)
{
	auto representatives_lk (representatives_mutex.lock ());
	representatives_lk.set (reps);
}

nano::wallet_representatives::wallet_representatives (nano::node & node_a) :
	handle{ rsnano::rsn_wallet_representatives_create (node_a.config->vote_minimum.bytes.data (), node_a.ledger.handle) }
{
}

nano::wallet_representatives::~wallet_representatives ()
{
	rsnano::rsn_wallet_representatives_destroy (handle);
}

uint64_t nano::wallet_representatives::voting_reps () const
{
	return rsnano::rsn_wallet_representatives_voting_reps (handle);
}

bool nano::wallet_representatives::have_half_rep () const
{
	return rsnano::rsn_wallet_representatives_have_half_rep (handle);
}

bool nano::wallet_representatives::exists (nano::account const & rep_a) const
{
	return rsnano::rsn_wallet_representatives_exists (handle, rep_a.bytes.data ());
}

void nano::wallet_representatives::clear ()
{
	return rsnano::rsn_wallet_representatives_clear (handle);
}

bool nano::wallet_representatives::check_rep (nano::account const & account_a, nano::uint128_t const & half_principal_weight_a)
{
	nano::amount half_weight{ half_principal_weight_a };
	return rsnano::rsn_wallet_representatives_check_rep (handle, account_a.bytes.data (), half_weight.bytes.data ());
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

std::shared_ptr<nano::wallet> nano::wallets::wallets_mutex_lock::find (nano::wallet_id const & wallet_id)
{
	rsnano::WalletHandle * wallet_handle = nullptr;
	std::shared_ptr<nano::wallet> wallet{};
	if (rsnano::rsn_lmdb_wallets_mutex_lock_find (handle, wallet_id.bytes.data (), &wallet_handle))
	{
		wallet = make_shared<nano::wallet> (wallet_handle);
	}
	return wallet;
}

void nano::wallets::wallets_mutex_lock::insert (nano::wallet_id const & wallet_id, std::shared_ptr<nano::wallet> wallet)
{
	rsnano::rsn_lmdb_wallets_mutex_lock_insert (handle, wallet_id.bytes.data (), wallet->handle);
}

std::unordered_map<nano::wallet_id, std::shared_ptr<nano::wallet>> nano::wallets::wallets_mutex_lock::get_all ()
{
	auto vec_handle = rsnano::rsn_lmdb_wallets_mutex_lock_get_all (handle);
	std::unordered_map<nano::wallet_id, std::shared_ptr<nano::wallet>> result;
	auto len = rsnano::rsn_wallet_vec_len (vec_handle);
	for (auto i = 0; i < len; ++i)
	{
		nano::wallet_id id;
		auto wallet_handle = rsnano::rsn_wallet_vec_get (vec_handle, i, id.bytes.data ());
		auto wallet{ std::make_shared<nano::wallet> (wallet_handle) };
		result.emplace (id, wallet);
	}
	rsnano::rsn_wallet_vec_destroy (vec_handle);
	return result;
}

size_t nano::wallets::wallets_mutex_lock::size () const
{
	return rsnano::rsn_lmdb_wallets_mutex_lock_size (handle);
}

void nano::wallets::wallets_mutex_lock::erase (nano::wallet_id const & wallet_id)
{
	rsnano::rsn_lmdb_wallets_mutex_lock_erase (handle, wallet_id.bytes.data ());
}

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

namespace
{
rsnano::LmdbWalletsHandle * create_wallets (nano::node & node_a, nano::store::lmdb::env & env)
{
	auto config_dto{ node_a.config->to_dto () };

	return rsnano::rsn_lmdb_wallets_create (
	node_a.config->enable_voting,
	env.handle,
	node_a.ledger.handle,
	&config_dto,
	node_a.config->network_params.kdf_work,
	&node_a.config->network_params.work.dto,
	node_a.distributed_work.handle);
}
}

nano::wallets::wallets (bool error_a, nano::node & node_a) :
	network_params{ node_a.config->network_params },
	kdf{ node_a.config->network_params.kdf_work },
	node (node_a),
	env (boost::polymorphic_downcast<nano::mdb_wallets_store *> (node_a.wallets_store_impl.get ())->environment),
	representatives{ node_a },
	rust_handle{ create_wallets (node_a, env) },
	mutex{ rust_handle },
	wallet_actions{}
{
	{
		// ported until here...

		auto lock{ mutex.lock () };
		auto wallets = lock.get_all ();
		for (auto & item : wallets)
		{
			enter_initial_password (item.second);
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
	return lock.size ();
}

size_t nano::wallets::representatives_count (nano::wallet_id const & id) const
{
	auto lock{ mutex.lock () };
	auto wallet = lock.find (id);
	return wallet->representatives_count ();
}

nano::key_type nano::wallets::key_type (nano::wallet_id const & wallet_id, nano::account const & account)
{
	auto lock{ mutex.lock () };
	auto wallet = lock.find (wallet_id);

	if (wallet == nullptr)
	{
		return nano::key_type::unknown;
	}

	auto txn{ tx_begin_read () };
	return wallet->store.key_type (*txn, account);
}

nano::wallets_error nano::wallets::get_representative (nano::wallet_id const & wallet_id, nano::account & representative)
{
	auto lock{ mutex.lock () };
	auto wallet = lock.find (wallet_id);

	if (wallet == nullptr)
	{
		return nano::wallets_error::wallet_not_found;
	}

	auto txn{ tx_begin_read () };
	representative = wallet->store.representative (*txn);
	return nano::wallets_error::none;
}

nano::wallets_error nano::wallets::set_representative (nano::wallet_id const & wallet_id, nano::account const & rep, bool update_existing_accounts)
{
	std::vector<nano::account> accounts;
	{
		auto lock{ mutex.lock () };
		auto wallet = lock.find (wallet_id);

		if (wallet == nullptr)
		{
			return nano::wallets_error::wallet_not_found;
		}

		{
			auto txn{ tx_begin_write () };
			if (update_existing_accounts && !wallet->store.valid_password (*txn))
			{
				return nano::wallets_error::wallet_locked;
			}

			wallet->store.representative_set (*txn, rep);
		}

		// Change representative for all wallet accounts
		if (update_existing_accounts)
		{
			auto txn{ tx_begin_read () };
			auto block_transaction (node.store.tx_begin_read ());
			for (auto i (wallet->store.begin (*txn)), n (wallet->store.end ()); i != n; ++i)
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
	auto wallet = lock.find (wallet_id);

	if (wallet == nullptr)
	{
		return nano::wallets_error::wallet_not_found;
	}

	auto txn{ tx_begin_read () };
	if (!wallet->store.valid_password (*txn))
	{
		return nano::wallets_error::wallet_locked;
	}

	wallet->store.seed (prv_a, *txn);
	return nano::wallets_error::none;
}

nano::wallets_error nano::wallets::change_seed (nano::wallet_id const & wallet_id, nano::raw_key const & prv_a, uint32_t count, nano::public_key & first_account, uint32_t & restored_count)
{
	auto lock{ mutex.lock () };
	auto wallet = lock.find (wallet_id);

	if (wallet == nullptr)
	{
		return nano::wallets_error::wallet_not_found;
	}

	auto txn{ tx_begin_write () };
	if (!wallet->store.valid_password (*txn))
	{
		return nano::wallets_error::wallet_locked;
	}

	first_account = change_seed (wallet, *txn, prv_a, count);
	restored_count = wallet->store.deterministic_index_get (*txn);
	return nano::wallets_error::none;
}

bool nano::wallets::ensure_wallet_is_unlocked (nano::wallet_id const & wallet_id, std::string const & password_a)
{
	auto lock{ mutex.lock () };
	auto existing{ lock.find (wallet_id) };
	bool valid (false);
	{
		auto transaction{ tx_begin_write () };
		valid = existing->store.valid_password (*transaction);
		if (!valid)
		{
			valid = !enter_password (existing, *transaction, password_a);
		}
	}
	return valid;
}

bool nano::wallets::import_replace (nano::wallet_id const & wallet_id, std::string const & json_a, std::string const & password_a)
{
	auto lock{ mutex.lock () };
	auto existing{ lock.find (wallet_id) };

	auto error (false);
	std::unique_ptr<nano::wallet_store> temp;
	{
		auto transaction (env.tx_begin_write ());
		nano::uint256_union id;
		random_pool::generate_block (id.bytes.data (), id.bytes.size ());
		temp = std::make_unique<nano::wallet_store> (error, kdf, *transaction, 0, 1, id.to_string (), json_a);
	}
	if (!error)
	{
		auto transaction (env.tx_begin_write ());
		error = temp->attempt_password (*transaction, password_a);
	}
	auto transaction (env.tx_begin_write ());
	if (!error)
	{
		error = existing->store.import (*transaction, *temp);
	}
	temp->destroy (*transaction);
	return error;
}

bool nano::wallets::import (nano::wallet_id const & wallet_id, std::string const & json_a)
{
	auto lock{ mutex.lock () };
	auto txn (tx_begin_write ());
	bool error = true;
	nano::wallet wallet (error, *txn, node.wallets, wallet_id.to_string (), json_a);
	return error;
}

nano::wallets_error nano::wallets::decrypt (nano::wallet_id const & wallet_id, std::vector<std::pair<nano::account, nano::raw_key>> & accounts) const
{
	auto lock{ mutex.lock () };
	auto wallet = lock.find (wallet_id);

	if (wallet == nullptr)
	{
		return nano::wallets_error::wallet_not_found;
	}

	auto txn{ tx_begin_read () };
	if (!wallet->store.valid_password (*txn))
	{
		return nano::wallets_error::wallet_locked;
	}

	for (auto i (wallet->store.begin (*txn)), m (wallet->store.end ()); i != m; ++i)
	{
		nano::account const & account (i->first);
		nano::raw_key key;
		auto error (wallet->store.fetch (*txn, account, key));
		(void)error;
		debug_assert (!error);
		accounts.emplace_back (account, key);
	}
	return nano::wallets_error::none;
}

nano::wallets_error nano::wallets::fetch (nano::wallet_id const & wallet_id, nano::account const & pub, nano::raw_key & prv)
{
	auto lock{ mutex.lock () };
	auto wallet = lock.find (wallet_id);

	if (wallet == nullptr)
	{
		return nano::wallets_error::wallet_not_found;
	}

	auto txn{ tx_begin_read () };
	if (!wallet->store.valid_password (*txn))
	{
		return nano::wallets_error::wallet_locked;
	}

	if (wallet->store.find (*txn, pub) == wallet->store.end ())
	{
		return nano::wallets_error::account_not_found;
	}

	if (wallet->store.fetch (*txn, pub, prv))
	{
		return nano::wallets_error::generic;
	}

	return nano::wallets_error::none;
}

std::vector<nano::wallet_id> nano::wallets::get_wallet_ids () const
{
	auto lock{ mutex.lock () };
	std::vector<nano::wallet_id> result{};
	result.reserve (lock.size ());
	auto wallets{ lock.get_all () };
	for (auto i (wallets.begin ()), n (wallets.end ()); i != n; ++i)
	{
		result.push_back (i->first);
	}
	return result;
}

nano::wallets_error nano::wallets::get_accounts (nano::wallet_id const & wallet_id, std::vector<nano::account> & accounts)
{
	auto lock{ mutex.lock () };
	auto wallet = lock.find (wallet_id);

	if (wallet == nullptr)
	{
		return nano::wallets_error::wallet_not_found;
	}
	auto txn (tx_begin_read ());

	for (auto j (wallet->store.begin (*txn)), m (wallet->store.end ()); j != m; ++j)
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
	auto wallets{ lock.get_all () };
	for (auto i (wallets.begin ()), n (wallets.end ()); i != n && accounts.size () < max_results; ++i)
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
	auto wallet (lock.find (wallet_id));
	if (wallet == nullptr)
	{
		return nano::wallets_error::wallet_not_found;
	}
	auto txn{ tx_begin_write () };
	if (wallet->store.find (*txn, account) == wallet->store.end ())
	{
		return nano::wallets_error::account_not_found;
	}
	wallet->store.work_get (*txn, account, work);
	return nano::wallets_error::none;
}

uint64_t nano::wallets::work_get (nano::wallet_id const & wallet_id, nano::account const & account)
{
	auto lock{ mutex.lock () };
	auto transaction (tx_begin_read ());
	auto wallet{ lock.find (wallet_id) };
	uint64_t work (1);
	wallet->store.work_get (*transaction, account, work);
	return work;
}

nano::wallets_error nano::wallets::work_set (nano::wallet_id const & wallet_id, nano::account const & account, uint64_t work)
{
	auto lock{ mutex.lock () };
	auto wallet (lock.find (wallet_id));
	if (wallet == nullptr)
	{
		return nano::wallets_error::wallet_not_found;
	}
	auto txn{ tx_begin_write () };
	if (wallet->store.find (*txn, account) == wallet->store.end ())
	{
		return nano::wallets_error::account_not_found;
	}

	wallet->store.work_put (*txn, account, work);
	return nano::wallets_error::none;
}

nano::wallets_error nano::wallets::remove_account (nano::wallet_id const & wallet_id, nano::account const & account_id)
{
	auto lock{ mutex.lock () };
	auto wallet (lock.find (wallet_id));
	if (wallet == nullptr)
	{
		return nano::wallets_error::wallet_not_found;
	}
	auto txn{ tx_begin_write () };
	if (!wallet->store.valid_password (*txn))
	{
		return nano::wallets_error::wallet_locked;
	}
	if (wallet->store.find (*txn, account_id) == wallet->store.end ())
	{
		return nano::wallets_error::account_not_found;
	}
	wallet->store.erase (*txn, account_id);
	return nano::wallets_error::none;
}

bool nano::wallets::move_accounts (nano::wallet_id const & source_id, nano::wallet_id const & target_id, std::vector<nano::public_key> const & accounts)
{
	auto lock{ mutex.lock () };
	auto existing (lock.find (source_id));
	auto source (existing);
	auto transaction (tx_begin_write ());
	auto target{ lock.find (target_id) };
	auto error (target->store.move (*transaction, source->store, accounts));
	return error;
}

bool nano::wallets::wallet_exists (nano::wallet_id const & id) const
{
	auto lock{ mutex.lock () };
	return lock.find (id) != nullptr;
}

nano::wallet_id nano::wallets::first_wallet_id () const
{
	auto lock{ mutex.lock () };
	auto wallets{ lock.get_all () };
	return wallets.begin ()->first;
}

nano::wallets_error nano::wallets::insert_adhoc (nano::wallet_id const & wallet_id, nano::raw_key const & key_a, bool generate_work_a)
{
	auto lock{ mutex.lock () };
	auto wallet (lock.find (wallet_id));
	if (wallet == nullptr)
	{
		return nano::wallets_error::wallet_not_found;
	}
	auto txn{ tx_begin_read () };
	if (!wallet->store.valid_password (*txn))
	{
		return nano::wallets_error::wallet_locked;
	}

	txn->reset ();
	insert_adhoc (wallet, key_a, generate_work_a);
	return nano::wallets_error::none;
}

nano::wallets_error nano::wallets::insert_adhoc (nano::wallet_id const & wallet_id, nano::raw_key const & key_a, bool generate_work_a, nano::public_key & account)
{
	auto lock{ mutex.lock () };
	auto wallet (lock.find (wallet_id));
	if (wallet == nullptr)
	{
		return nano::wallets_error::wallet_not_found;
	}
	auto txn{ tx_begin_read () };
	if (!wallet->store.valid_password (*txn))
	{
		return nano::wallets_error::wallet_locked;
	}
	txn->reset ();
	account = insert_adhoc (wallet, key_a, generate_work_a);
	return nano::wallets_error::none;
}

nano::public_key nano::wallets::insert_adhoc (const std::shared_ptr<nano::wallet> & wallet, nano::raw_key const & key_a, bool generate_work_a)
{
	nano::public_key key{};
	auto transaction (env.tx_begin_write ());
	if (wallet->store.valid_password (*transaction))
	{
		key = wallet->store.insert_adhoc (*transaction, key_a);
		auto block_transaction (node.store.tx_begin_read ());
		if (generate_work_a)
		{
			work_ensure (wallet, key, node.ledger.latest_root (*block_transaction, key));
		}
		auto half_principal_weight (node.minimum_principal_weight () / 2);
		// Makes sure that the representatives container will
		// be in sync with any added keys.
		transaction->commit ();
		auto lock{ nano::unique_lock<nano::mutex>{ reps_cache_mutex } };
		if (representatives.check_rep (key, half_principal_weight))
		{
			wallet->insert_representative (key);
		}
	}
	return key;
}

std::shared_ptr<nano::block> nano::wallets::receive_action (const std::shared_ptr<nano::wallet> & wallet, nano::block_hash const & send_hash_a, nano::account const & representative_a, nano::uint128_union const & amount_a, nano::account const & account_a, uint64_t work_a, bool generate_work_a)
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
				if (!wallet->store.fetch (*transaction, account_a, prv))
				{
					if (work_a == 0)
					{
						wallet->store.work_get (*transaction, account_a, work_a);
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
					node.logger->warn (nano::log::type::wallet, "Unable to receive, wallet locked");
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
		node.logger->warn (nano::log::type::wallet, "Not receiving block {} due to minimum receive threshold", send_hash_a.to_string ());
		// Someone sent us something below the threshold of receiving
	}
	if (block != nullptr)
	{
		auto details = nano::block_details (epoch, false, true, false);
		if (action_complete (wallet, block, account_a, generate_work_a, details))
		{
			// Return null block after work generation or ledger process error
			block = nullptr;
		}
	}
	return block;
}

std::shared_ptr<nano::block> nano::wallets::change_action (const std::shared_ptr<wallet> & wallet, nano::account const & source_a, nano::account const & representative_a, uint64_t work_a, bool generate_work_a)
{
	auto epoch = nano::epoch::epoch_0;
	std::shared_ptr<nano::block> block;
	{
		auto transaction (env.tx_begin_read ());
		auto block_transaction (node.store.tx_begin_read ());
		if (wallet->store.valid_password (*transaction))
		{
			auto existing (wallet->store.find (*transaction, source_a));
			if (existing != wallet->store.end () && !node.ledger.latest (*block_transaction, source_a).is_zero ())
			{
				auto info = node.ledger.account_info (*block_transaction, source_a);
				debug_assert (info);
				nano::raw_key prv;
				auto error2 (wallet->store.fetch (*transaction, source_a, prv));
				(void)error2;
				debug_assert (!error2);
				if (work_a == 0)
				{
					wallet->store.work_get (*transaction, source_a, work_a);
				}
				block = std::make_shared<nano::state_block> (source_a, info->head (), representative_a, info->balance (), 0, prv, source_a, work_a);
				epoch = info->epoch ();
			}
		}
	}
	if (block != nullptr)
	{
		auto details = nano::block_details (epoch, false, false, false);
		if (action_complete (wallet, block, source_a, generate_work_a, details))
		{
			// Return null block after work generation or ledger process error
			block = nullptr;
		}
	}
	return block;
}

std::shared_ptr<nano::block> nano::wallets::send_action (const std::shared_ptr<nano::wallet> & wallet, nano::account const & source_a, nano::account const & account_a, nano::uint128_t const & amount_a, uint64_t work_a, bool generate_work_a, boost::optional<std::string> id_a)
{
	auto prepare_send = [&id_a, &node = this->node, &wallets = *this, &store = wallet->store, &source_a, &amount_a, &work_a, &account_a] (auto const & transaction) {
		auto block_transaction (node.store.tx_begin_read ());

		auto error (false);
		std::shared_ptr<nano::block> block;
		if (id_a)
		{
			auto hash{ wallets.get_block_hash (error, *transaction, *id_a) };
			if (!hash.is_zero ())
			{
				block = node.ledger.block (*block_transaction, hash);
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
		if (action_complete (wallet, block, source_a, generate_work_a, details))
		{
			// Return null block after work generation or ledger process error
			block = nullptr;
		}
	}
	return block;
}

void nano::wallets::change_async (const std::shared_ptr<nano::wallet> & wallet, nano::account const & source_a, nano::account const & representative_a, std::function<void (std::shared_ptr<nano::block> const &)> const & action_a, uint64_t work_a, bool generate_work_a)
{
	wallet_actions.queue_wallet_action (nano::wallets::high_priority, wallet, [&this_l = *this, wallet, source_a, representative_a, action_a, work_a, generate_work_a] (nano::wallet & wallet_a) {
		auto block (this_l.change_action (wallet, source_a, representative_a, work_a, generate_work_a));
		action_a (block);
	});
}

bool nano::wallets::change_sync (const std::shared_ptr<nano::wallet> & wallet, nano::account const & source_a, nano::account const & representative_a)
{
	std::promise<bool> result;
	std::future<bool> future = result.get_future ();
	change_async (
	wallet,
	source_a, representative_a, [&result] (std::shared_ptr<nano::block> const & block_a) {
		result.set_value (block_a == nullptr);
	},
	0, true);
	return future.get ();
}

void nano::wallets::receive_async (const std::shared_ptr<nano::wallet> & wallet, nano::block_hash const & hash_a, nano::account const & representative_a, nano::uint128_t const & amount_a, nano::account const & account_a, std::function<void (std::shared_ptr<nano::block> const &)> const & action_a, uint64_t work_a, bool generate_work_a)
{
	wallet_actions.queue_wallet_action (amount_a, wallet, [&this_l = *this, wallet, hash_a, representative_a, amount_a, account_a, action_a, work_a, generate_work_a] (nano::wallet & wallet_a) {
		auto block (this_l.receive_action (wallet, hash_a, representative_a, amount_a, account_a, work_a, generate_work_a));
		action_a (block);
	});
}

bool nano::wallets::receive_sync (const std::shared_ptr<nano::wallet> & wallet, std::shared_ptr<nano::block> const & block_a, nano::account const & representative_a, nano::uint128_t const & amount_a)
{
	std::promise<bool> result;
	std::future<bool> future = result.get_future ();
	receive_async (
	wallet,
	block_a->hash (), representative_a, amount_a, block_a->destination (), [&result] (std::shared_ptr<nano::block> const & block_a) {
		result.set_value (block_a == nullptr);
	},
	0, true);
	return future.get ();
}

bool nano::wallets::search_receivable (const std::shared_ptr<nano::wallet> & wallet, store::transaction const & wallet_transaction_a)
{
	std::function<void (std::shared_ptr<nano::block> const &)> start_election =
	[&this_l = *this] (std::shared_ptr<nano::block> const & block) {
		this_l.node.start_election (block);
	};

	// TODO port this:
	auto error (!wallet->store.valid_password (wallet_transaction_a));
	if (!error)
	{
		node.logger->info (nano::log::type::wallet, "Beginning receivable block search");

		for (auto i (wallet->store.begin (wallet_transaction_a)), n (wallet->store.end ()); i != n; ++i)
		{
			auto block_transaction (node.store.tx_begin_read ());
			nano::account const & account (i->first);
			// Don't search pending for watch-only accounts
			if (!nano::wallet_value (i->second).key.is_zero ())
			{
				for (auto i = node.ledger.receivable_upper_bound (*block_transaction, account, 0); !i.is_end (); ++i)
				{
					auto const & [key, info] = *i;
					auto hash = key.hash;

					auto amount = info.amount.number ();
					if (node.config->receive_minimum.number () <= amount)
					{
						node.logger->info (nano::log::type::wallet, "Found a receivable block {} for account {}", hash.to_string (), info.source.to_account ());
						if (node.ledger.block_confirmed (*block_transaction, hash))
						{
							auto representative = wallet->store.representative (wallet_transaction_a);
							// Receive confirmed block
							receive_async (
							wallet, hash, representative, amount, account, [] (std::shared_ptr<nano::block> const &) {}, 0, true);
						}
						else if (!node.confirming_set.exists (hash))
						{
							auto block (node.ledger.block (*block_transaction, hash));
							if (block)
							{
								// Request confirmation for block which is not being processed yet
								start_election (block);
							}
						}
					}
				}
			}
		}
		node.logger->info (nano::log::type::wallet, "Receivable block search phase completed");
	}
	else
	{
		node.logger->info (nano::log::type::wallet, "Stopping search, wallet is locked");
	}
	return error;
}

bool nano::wallets::enter_password (const std::shared_ptr<nano::wallet> & wallet, store::transaction const & transaction_a, std::string const & password_a)
{
	auto error (wallet->store.attempt_password (transaction_a, password_a));
	if (!error)
	{
		node.logger->info (nano::log::type::wallet, "Wallet unlocked");

		wallet_actions.queue_wallet_action (nano::wallets::high_priority, wallet, [&this_l = *this] (nano::wallet & wallet) {
			// Wallets must survive node lifetime
			auto tx{ this_l.tx_begin_read () };
			this_l.search_receivable (wallet.shared_from_this (), *tx);
		});
	}
	else
	{
		node.logger->warn (nano::log::type::wallet, "Invalid password, wallet locked");
	}
	return error;
}

void nano::wallets::enter_initial_password (const std::shared_ptr<nano::wallet> & wallet)
{
	nano::raw_key password_l;
	wallet->store.password (password_l);
	if (password_l.is_zero ())
	{
		auto transaction (env.tx_begin_write ());
		if (wallet->store.valid_password (*transaction))
		{
			// Newly created wallets have a zero key
			wallet->store.rekey (*transaction, "");
		}
		else
		{
			enter_password (wallet, *transaction, "");
		}
	}
}

nano::public_key nano::wallets::change_seed (const std::shared_ptr<nano::wallet> & wallet, store::transaction const & transaction_a, nano::raw_key const & prv_a, uint32_t count)
{
	wallet->store.seed_set (transaction_a, prv_a);
	auto account = deterministic_insert (wallet, transaction_a, true);
	if (count == 0)
	{
		count = wallet->deterministic_check (transaction_a, 0);
	}
	for (uint32_t i (0); i < count; ++i)
	{
		// Disable work generation to prevent weak CPU nodes stuck
		account = deterministic_insert (wallet, transaction_a, false);
	}
	return account;
}

void nano::wallets::work_cache_blocking (const std::shared_ptr<nano::wallet> & wallet, nano::account const & account_a, nano::root const & root_a)
{
	if (node.distributed_work.work_generation_enabled ())
	{
		auto difficulty (node.default_difficulty (nano::work_version::work_1));
		auto opt_work_l (node.distributed_work.make_blocking (nano::work_version::work_1, root_a, difficulty, account_a));
		if (opt_work_l.has_value ())
		{
			auto transaction_l (env.tx_begin_write ());
			if (wallet->live () && wallet->store.exists (*transaction_l, account_a))
			{
				wallet->work_update (*transaction_l, account_a, root_a, opt_work_l.value ());
			}
		}
		else if (!node.stopped)
		{
			node.logger->warn (nano::log::type::wallet, "Could not precache work for root {} due to work generation failure", root_a.to_string ());
		}
	}
}

nano::wallets_error nano::wallets::insert_watch (nano::wallet_id const & wallet_id, std::vector<nano::public_key> const & accounts)
{
	auto lock{ mutex.lock () };
	auto wallet (lock.find (wallet_id));
	if (wallet == nullptr)
	{
		return nano::wallets_error::wallet_not_found;
	}
	auto txn{ tx_begin_write () };
	if (!wallet->store.valid_password (*txn))
	{
		return nano::wallets_error::wallet_locked;
	}

	for (auto & account : accounts)
	{
		if (wallet->insert_watch (*txn, account))
		{
			return nano::wallets_error::bad_public_key;
		}
	}

	return nano::wallets_error::none;
}

void nano::wallets::set_password (nano::wallet_id const & wallet_id, nano::raw_key const & password)
{
	auto lock{ mutex.lock () };
	auto wallet{ lock.find (wallet_id) };
	wallet->store.set_password (password);
}

void nano::wallets::password (nano::wallet_id const & wallet_id, nano::raw_key & password_a) const
{
	auto lock{ mutex.lock () };
	auto wallet{ lock.find (wallet_id) };
	wallet->store.password (password_a);
}

nano::wallets_error nano::wallets::enter_password (nano::wallet_id const & wallet_id, std::string const & password_a)
{
	auto lock{ mutex.lock () };
	auto wallet (lock.find (wallet_id));
	if (wallet == nullptr)
	{
		return nano::wallets_error::wallet_not_found;
	}
	auto txn{ tx_begin_write () };

	bool error = enter_password (wallet, *txn, password_a);
	if (error)
	{
		return nano::wallets_error::invalid_password;
	}
	return nano::wallets_error::none;
}

void nano::wallets::enter_initial_password (nano::wallet_id const & wallet_id)
{
	auto lock{ mutex.lock () };
	auto wallet{ lock.find (wallet_id) };
	enter_initial_password (wallet);
}

nano::wallets_error nano::wallets::valid_password (nano::wallet_id const & wallet_id, bool & valid)
{
	auto lock{ mutex.lock () };
	auto wallet (lock.find (wallet_id));
	if (wallet == nullptr)
	{
		return nano::wallets_error::wallet_not_found;
	}
	auto txn{ tx_begin_read () };
	valid = wallet->store.valid_password (*txn);
	return nano::wallets_error::none;
}

nano::wallets_error nano::wallets::attempt_password (nano::wallet_id const & wallet_id, std::string const & password, bool & error)
{
	auto lock{ mutex.lock () };
	auto wallet (lock.find (wallet_id));
	if (wallet == nullptr)
	{
		return nano::wallets_error::wallet_not_found;
	}
	auto txn{ tx_begin_write () };
	error = wallet->store.attempt_password (*txn, password);
	return nano::wallets_error::none;
}

nano::wallets_error nano::wallets::rekey (nano::wallet_id const wallet_id, std::string const & password)
{
	auto lock{ mutex.lock () };
	auto wallet (lock.find (wallet_id));
	if (wallet == nullptr)
	{
		return nano::wallets_error::wallet_not_found;
	}
	auto txn{ tx_begin_write () };
	if (!wallet->store.valid_password (*txn))
	{
		return nano::wallets_error::wallet_locked;
	}

	if (wallet->store.rekey (*txn, password))
	{
		return nano::wallets_error::generic;
	}
	return nano::wallets_error::none;
}

nano::wallets_error nano::wallets::lock (nano::wallet_id const & wallet_id)
{
	auto lock{ mutex.lock () };
	auto wallet (lock.find (wallet_id));
	if (wallet == nullptr)
	{
		return nano::wallets_error::wallet_not_found;
	}
	wallet->store.lock ();
	return nano::wallets_error::none;
}

nano::public_key nano::wallets::deterministic_insert (const std::shared_ptr<nano::wallet> & wallet, store::transaction const & transaction_a, bool generate_work_a)
{
	nano::public_key key{};
	if (wallet->store.valid_password (transaction_a))
	{
		key = wallet->store.deterministic_insert (transaction_a);
		if (generate_work_a)
		{
			work_ensure (wallet, key, key);
		}
		auto half_principal_weight (node.minimum_principal_weight () / 2);
		auto lock{ nano::unique_lock<nano::mutex>{ reps_cache_mutex } };
		if (representatives.check_rep (key, half_principal_weight))
		{
			wallet->insert_representative (key);
		}
	}
	return key;
}

nano::wallets_error nano::wallets::deterministic_insert (nano::wallet_id const & wallet_id, uint32_t const index, bool generate_work_a, nano::account & account)
{
	auto lock{ mutex.lock () };
	auto wallet (lock.find (wallet_id));
	if (wallet == nullptr)
	{
		return nano::wallets_error::wallet_not_found;
	}
	auto txn{ tx_begin_write () };
	if (!wallet->store.valid_password (*txn))
	{
		return nano::wallets_error::wallet_locked;
	}

	account = wallet->store.deterministic_insert (*txn, index);
	if (generate_work_a)
	{
		work_ensure (wallet, account, account);
	}
	return nano::wallets_error::none;
}

nano::wallets_error nano::wallets::deterministic_insert (nano::wallet_id const & wallet_id, bool generate_work_a, nano::account & account)
{
	auto lock{ mutex.lock () };
	auto wallet (lock.find (wallet_id));
	if (wallet == nullptr)
	{
		return nano::wallets_error::wallet_not_found;
	}
	auto txn{ tx_begin_write () };
	if (!wallet->store.valid_password (*txn))
	{
		return nano::wallets_error::wallet_locked;
	}

	account = deterministic_insert (wallet, *txn, generate_work_a);
	return nano::wallets_error::none;
}

nano::wallets_error nano::wallets::deterministic_index_get (nano::wallet_id const & wallet_id, uint32_t & index)
{
	index = 0;
	auto lock{ mutex.lock () };
	auto wallet (lock.find (wallet_id));
	if (wallet == nullptr)
	{
		return nano::wallets_error::wallet_not_found;
	}
	auto txn{ tx_begin_read () };
	index = wallet->store.deterministic_index_get (*txn);
	return nano::wallets_error::none;
}

void nano::wallets::backup (std::filesystem::path const & backup_path)
{
	auto lock{ mutex.lock () };
	auto transaction{ tx_begin_read () };
	auto wallets{ lock.get_all () };
	for (auto i (wallets.begin ()), n (wallets.end ()); i != n; ++i)
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
	auto wallet{ lock.find (wallet_id) };
	work_cache_blocking (wallet, account_a, root_a);
}

std::shared_ptr<nano::block> nano::wallets::send_action (nano::wallet_id const & wallet_id, nano::account const & source_a, nano::account const & account_a, nano::uint128_t const & amount_a, uint64_t work_a, bool generate_work_a, boost::optional<std::string> id_a)
{
	auto lock{ mutex.lock () };
	auto wallet = lock.find (wallet_id);
	return send_action (wallet, source_a, account_a, amount_a, work_a, generate_work_a, id_a);
}

std::shared_ptr<nano::block> nano::wallets::receive_action (nano::wallet_id const & wallet_id, nano::block_hash const & send_hash_a, nano::account const & representative_a, nano::uint128_union const & amount_a, nano::account const & account_a, uint64_t work_a, bool generate_work_a)
{
	auto lock{ mutex.lock () };
	auto wallet = lock.find (wallet_id);
	return receive_action (wallet, send_hash_a, representative_a, amount_a, account_a, work_a, generate_work_a);
}

std::shared_ptr<nano::block> nano::wallets::change_action (nano::wallet_id const & wallet_id, nano::account const & source_a, nano::account const & representative_a, uint64_t work_a, bool generate_work_a)
{
	auto lock{ mutex.lock () };
	auto wallet = lock.find (wallet_id);
	return change_action (wallet, source_a, representative_a, work_a, generate_work_a);
}

nano::block_hash nano::wallets::send_sync (nano::wallet_id const & wallet_id, nano::account const & source_a, nano::account const & account_a, nano::uint128_t const & amount_a)
{
	auto lock{ mutex.lock () };
	auto wallet = lock.find (wallet_id);

	std::promise<nano::block_hash> result;
	std::future<nano::block_hash> future = result.get_future ();
	send_async (wallet, source_a, account_a, amount_a, [&result] (std::shared_ptr<nano::block> const & block_a) {
		result.set_value (block_a->hash ());
	},
	0, true, {});
	return future.get ();
}

bool nano::wallets::receive_sync (nano::wallet_id const & wallet_id, std::shared_ptr<nano::block> const & block_a, nano::account const & representative_a, nano::uint128_t const & amount_a)
{
	auto lock{ mutex.lock () };
	auto wallet = lock.find (wallet_id);
	return receive_sync (wallet, block_a, representative_a, amount_a);
}

bool nano::wallets::change_sync (nano::wallet_id const & wallet_id, nano::account const & source_a, nano::account const & representative_a)
{
	auto lock{ mutex.lock () };
	auto wallet = lock.find (wallet_id);
	return change_sync (wallet, source_a, representative_a);
}

nano::wallets_error nano::wallets::receive_async (nano::wallet_id const & wallet_id, nano::block_hash const & hash_a, nano::account const & representative_a, nano::uint128_t const & amount_a, nano::account const & account_a, std::function<void (std::shared_ptr<nano::block> const &)> const & action_a, uint64_t work_a, bool generate_work_a)
{
	auto lock{ mutex.lock () };
	auto wallet (lock.find (wallet_id));
	if (wallet == nullptr)
	{
		return nano::wallets_error::wallet_not_found;
	}
	auto txn{ tx_begin_write () };
	if (!wallet->store.valid_password (*txn))
	{
		return nano::wallets_error::wallet_locked;
	}
	if (wallet->store.find (*txn, account_a) == wallet->store.end ())
	{
		return nano::wallets_error::account_not_found;
	}

	receive_async (wallet, hash_a, representative_a, amount_a, account_a, action_a, work_a, generate_work_a);
	return nano::wallets_error::none;
}

nano::wallets_error nano::wallets::change_async (nano::wallet_id const & wallet_id, nano::account const & source_a, nano::account const & representative_a, std::function<void (std::shared_ptr<nano::block> const &)> const & action_a, uint64_t work_a, bool generate_work_a)
{
	auto lock{ mutex.lock () };
	auto wallet (lock.find (wallet_id));
	if (wallet == nullptr)
	{
		return nano::wallets_error::wallet_not_found;
	}
	auto txn{ tx_begin_write () };
	if (!wallet->store.valid_password (*txn))
	{
		return nano::wallets_error::wallet_locked;
	}
	if (wallet->store.find (*txn, source_a) == wallet->store.end ())
	{
		return nano::wallets_error::account_not_found;
	}
	change_async (wallet, source_a, representative_a, action_a, work_a, generate_work_a);
	return nano::wallets_error::none;
}

nano::wallets_error nano::wallets::send_async (nano::wallet_id const & wallet_id, nano::account const & source_a, nano::account const & account_a, nano::uint128_t const & amount_a, std::function<void (std::shared_ptr<nano::block> const &)> const & action_a, uint64_t work_a, bool generate_work_a, boost::optional<std::string> id_a)
{
	auto lock{ mutex.lock () };
	auto wallet (lock.find (wallet_id));
	if (wallet == nullptr)
	{
		return nano::wallets_error::wallet_not_found;
	}
	auto txn{ tx_begin_write () };
	if (!wallet->store.valid_password (*txn))
	{
		return nano::wallets_error::wallet_locked;
	}
	if (wallet->store.find (*txn, source_a) == wallet->store.end ())
	{
		return nano::wallets_error::account_not_found;
	}

	send_async (wallet, source_a, account_a, amount_a, action_a, work_a, generate_work_a, id_a);
	return nano::wallets_error::none;
}

void nano::wallets::receive_confirmed (store::transaction const & block_transaction_a, nano::block_hash const & hash_a, nano::account const & destination_a)
{
	std::unordered_map<nano::wallet_id, std::shared_ptr<nano::wallet>> wallets_l;
	std::unique_ptr<nano::store::read_transaction> wallet_transaction;
	{
		auto lk{ mutex.lock () };
		wallets_l = lk.get_all ();
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
				receive_async (
				wallet, hash_a, representative, amount, destination_a, [] (std::shared_ptr<nano::block> const &) {}, 0, true);
			}
			else
			{
				if (!node.ledger.block_or_pruned_exists (block_transaction_a, hash_a))
				{
					node.logger->warn (nano::log::type::wallet, "Confirmed block is missing:  {}", hash_a.to_string ());
					debug_assert (false && "Confirmed block is missing");
				}
				else
				{
					node.logger->warn (nano::log::type::wallet, "Block %1% has already been received: {}", hash_a.to_string ());
				}
			}
		}
	}
}

nano::wallets_error nano::wallets::serialize (nano::wallet_id const & wallet_id, std::string & json)
{
	auto lock{ mutex.lock () };
	auto wallet (lock.find (wallet_id));
	if (wallet == nullptr)
	{
		return nano::wallets_error::wallet_not_found;
	}
	auto txn{ tx_begin_read () };
	wallet->store.serialize_json (*txn, json);
	return nano::wallets_error::none;
}

void nano::wallets::create (nano::wallet_id const & id_a)
{
	auto lock{ mutex.lock () };
	debug_assert (lock.find (id_a) == nullptr);
	std::shared_ptr<nano::wallet> result;
	bool error = false;
	{
		auto transaction (tx_begin_write ());
		result = std::make_shared<nano::wallet> (error, *transaction, *this, id_a.to_string ());
	}
	if (!error)
	{
		lock.insert (id_a, result);
		enter_initial_password (result);
	}
}

nano::wallets_error nano::wallets::search_receivable (nano::wallet_id const & wallet_id)
{
	auto lock{ mutex.lock () };
	auto wallet (lock.find (wallet_id));
	if (wallet == nullptr)
	{
		return nano::wallets_error::wallet_not_found;
	}
	auto txn{ tx_begin_read () };
	if (!wallet->store.valid_password (*txn))
	{
		return nano::wallets_error::wallet_locked;
	}

	search_receivable (wallet, *txn);
	return nano::wallets_error::none;
}

void nano::wallets::search_receivable_all ()
{
	std::unordered_map<nano::wallet_id, std::shared_ptr<nano::wallet>> wallets_l;
	{
		auto lk{ mutex.lock () };
		wallets_l = lk.get_all ();
	}
	auto wallet_transaction (tx_begin_read ());
	for (auto const & [id, wallet] : wallets_l)
	{
		search_receivable (wallet, *wallet_transaction);
	}
}

void nano::wallets::destroy (nano::wallet_id const & id_a)
{
	auto lock{ mutex.lock () };
	auto transaction (tx_begin_write ());
	// action_mutex should be locked after transactions to prevent deadlocks in deterministic_insert () & insert_adhoc ()
	auto action_lock{ wallet_actions.lock () };
	auto existing (lock.find (id_a));
	debug_assert (existing != nullptr);
	auto wallet (existing);
	lock.erase (id_a);
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
		if (lock.find (id) == nullptr)
		{
			bool error = false;
			std::string text;
			id.encode_hex (text);
			auto wallet (std::make_shared<nano::wallet> (error, *transaction, *this, text));
			if (!error)
			{
				lock.insert (id, wallet);
			}
		}
		// List of wallets on disk
		stored_items.insert (id);
	}
	// Delete non existing wallets from memory
	std::vector<nano::wallet_id> deleted_items;
	auto wallets{ lock.get_all () };
	for (auto i : wallets)
	{
		if (stored_items.find (i.first) == stored_items.end ())
		{
			deleted_items.push_back (i.first);
		}
	}
	for (auto & i : deleted_items)
	{
		debug_assert (lock.find (i) == nullptr);
		lock.erase (i);
	}
}

namespace
{
void foreach_representative_action (void * context, const uint8_t * pub_key_bytes, const uint8_t * priv_key_bytes)
{
	auto action = static_cast<std::function<void (nano::public_key const & pub_a, nano::raw_key const & prv_a)> *> (context);
	auto pub_key{ nano::public_key::from_bytes (pub_key_bytes) };
	auto prv_key{ nano::raw_key::from_bytes (priv_key_bytes) };
	(*action) (pub_key, prv_key);
}

void delete_foreach_representative_context (void * context)
{
	auto action = static_cast<std::function<void (nano::public_key const & pub_a, nano::raw_key const & prv_a)> *> (context);
	delete action;
}
}

void nano::wallets::foreach_representative (std::function<void (nano::public_key const & pub_a, nano::raw_key const & prv_a)> const & action_a)
{
	auto context = new std::function<void (nano::public_key const & pub_a, nano::raw_key const & prv_a)> (action_a);
	rsnano::rsn_wallets_foreach_representative (
	rust_handle,
	foreach_representative_action,
	context,
	delete_foreach_representative_context);
}

bool nano::wallets::exists (nano::account const & account_a)
{
	auto lock{ mutex.lock () };
	auto txn{ tx_begin_read () };
	auto result (false);
	auto wallets{ lock.get_all () };
	for (auto i (wallets.begin ()), n (wallets.end ()); !result && i != n; ++i)
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
	nano::lock_guard<nano::mutex> counts_guard{ reps_cache_mutex };
	return representatives.voting_reps ();
}

bool nano::wallets::have_half_rep () const
{
	nano::lock_guard<nano::mutex> counts_guard{ reps_cache_mutex };
	return representatives.have_half_rep ();
}

bool nano::wallets::rep_exists (nano::account const & rep) const
{
	nano::lock_guard<nano::mutex> counts_guard{ reps_cache_mutex };
	return representatives.exists (rep);
}

bool nano::wallets::should_republish_vote (nano::account const & voting_account) const
{
	nano::lock_guard<nano::mutex> counts_guard{ reps_cache_mutex };
	return !representatives.have_half_rep () && !representatives.exists (voting_account);
}

void nano::wallets::compute_reps ()
{
	auto guard{ mutex.lock () };
	nano::lock_guard<nano::mutex> counts_guard{ reps_cache_mutex };
	representatives.clear ();
	auto half_principal_weight (node.minimum_principal_weight () / 2);
	auto transaction (tx_begin_read ());
	auto wallets{ guard.get_all () };
	for (auto i (wallets.begin ()), n (wallets.end ()); i != n; ++i)
	{
		auto & wallet (*i->second);
		std::unordered_set<nano::account> representatives_l;
		for (auto ii (wallet.store.begin (*transaction)), nn (wallet.store.end ()); ii != nn; ++ii)
		{
			auto account (ii->first);
			if (representatives.check_rep (account, half_principal_weight))
			{
				representatives_l.insert (account);
			}
		}
		wallet.set_representatives (representatives_l);
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

void nano::wallets::send_async (const std::shared_ptr<nano::wallet> & wallet, nano::account const & source_a, nano::account const & account_a, nano::uint128_t const & amount_a, std::function<void (std::shared_ptr<nano::block> const &)> const & action_a, uint64_t work_a, bool generate_work_a, boost::optional<std::string> id_a)
{
	wallet_actions.queue_wallet_action (nano::wallets::high_priority, wallet, [&wallets = *this, wallet, source_a, account_a, amount_a, action_a, work_a, generate_work_a, id_a] (nano::wallet & wallet_a) {
		auto block{ wallets.send_action (wallet, source_a, account_a, amount_a, work_a, generate_work_a, id_a) };
		action_a (block);
	});
}

void nano::wallets::work_ensure (std::shared_ptr<nano::wallet> wallet, nano::account const & account_a, nano::root const & root_a)
{
	using namespace std::chrono_literals;
	std::chrono::seconds const precache_delay = node.network_params.network.is_dev_network () ? 1s : 10s;

	delayed_work->operator[] (account_a) = root_a;

	node.workers->add_timed_task (std::chrono::steady_clock::now () + precache_delay, [&this_l = *this, wallet_l = wallet, account_a, root_a] {
		auto delayed_work = this_l.delayed_work.lock ();
		auto existing (delayed_work->find (account_a));
		if (existing != delayed_work->end () && existing->second == root_a)
		{
			delayed_work->erase (existing);
			this_l.wallet_actions.queue_wallet_action (nano::wallets::generate_priority, wallet_l, [account_a, root_a, wallet_l, &this_l] (nano::wallet & wallet_a) {
				this_l.work_cache_blocking (wallet_l, account_a, root_a);
			});
		}
	});
}

bool nano::wallets::action_complete (std::shared_ptr<nano::wallet> wallet, std::shared_ptr<nano::block> const & block_a, nano::account const & account_a, bool const generate_work_a, nano::block_details const & details_a)
{
	bool error{ false };
	// Unschedule any work caching for this account
	delayed_work->erase (account_a);
	if (block_a != nullptr)
	{
		auto required_difficulty{ node.network_params.work.threshold (block_a->work_version (), details_a) };
		if (node.network_params.work.difficulty (*block_a) < required_difficulty)
		{
			node.logger->info (nano::log::type::wallet, "Cached or provided work for block {} account {} is invalid, regenerating...",
			block_a->hash ().to_string (),
			account_a.to_account ());

			debug_assert (required_difficulty <= node.max_work_generate_difficulty (block_a->work_version ()));
			error = !node.work_generate_blocking (*block_a, required_difficulty).has_value ();
		}
		if (!error)
		{
			auto result = node.process_local (block_a);
			error = !result || result.value () != nano::block_status::progress;
			debug_assert (error || block_a->sideband ().details () == details_a);
		}
		if (!error && generate_work_a)
		{
			// Pregenerate work for next block based on the block just created
			work_ensure (wallet, account_a, block_a->hash ());
		}
	}
	return error;
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
		items_count = guard.size ();
	}

	auto sizeof_item_element = sizeof (nano::wallet_id) + sizeof (uintptr_t);
	auto sizeof_actions_element = sizeof (uintptr_t) * 2;
	auto composite = std::make_unique<container_info_composite> (name);
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "items", items_count, sizeof_item_element }));
	composite->add_component (std::make_unique<container_info_leaf> (container_info{ "actions", actions_count, sizeof_actions_element }));
	return composite;
}
