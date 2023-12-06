#pragma once

#include "boost/none.hpp"
#include "nano/lib/rsnano.hpp"

#include <nano/lib/lmdbconfig.hpp>
#include <nano/lib/locks.hpp>
#include <nano/lib/work.hpp>
#include <nano/node/openclwork.hpp>
#include <nano/secure/common.hpp>
#include <nano/store/component.hpp>
#include <nano/store/lmdb/lmdb.hpp>
#include <nano/store/lmdb/wallet_value.hpp>

#include <atomic>
#include <mutex>
#include <thread>
#include <unordered_set>
namespace rsnano
{
class KdfHandle;
}
namespace nano
{
class node;
class node_config;
class wallets;
class wallet_action_thread;
class wallet_representatives;
class kdf final
{
public:
	kdf (unsigned kdf_work);
	kdf (kdf const &) = delete;
	kdf (kdf &&) = delete;
	~kdf ();
	void phs (nano::raw_key &, std::string const &, nano::uint256_union const &);
	rsnano::KdfHandle * handle;
};
enum class key_type
{
	not_a_type,
	unknown,
	adhoc,
	deterministic
};
class wallet_store final
{
public:
	wallet_store (bool &, nano::kdf &, store::transaction &, nano::account, unsigned, std::string const &);
	wallet_store (bool &, nano::kdf &, store::transaction &, nano::account, unsigned, std::string const &, std::string const &);
	~wallet_store ();
	wallet_store (wallet_store const &) = delete;
	bool is_open () const;
	void lock ();
	void password (nano::raw_key & password_a) const;
	void set_password (nano::raw_key const & password_a);
	std::vector<nano::account> accounts (store::transaction const &);
	nano::uint256_union check (store::transaction const &);
	bool rekey (store::transaction const &, std::string const &);
	bool valid_password (store::transaction const &);
	bool attempt_password (store::transaction const &, std::string const &);
	void wallet_key (nano::raw_key &, store::transaction const &);
	void seed (nano::raw_key &, store::transaction const &);
	void seed_set (store::transaction const &, nano::raw_key const &);
	nano::key_type key_type (nano::wallet_value const &);
	nano::public_key deterministic_insert (store::transaction const &);
	nano::public_key deterministic_insert (store::transaction const &, uint32_t const);
	nano::raw_key deterministic_key (store::transaction const &, uint32_t);
	uint32_t deterministic_index_get (store::transaction const &);
	void deterministic_index_set (store::transaction const &, uint32_t);
	void deterministic_clear (store::transaction const &);
	nano::uint256_union salt (store::transaction const &);
	bool is_representative (store::transaction const &);
	nano::account representative (store::transaction const &);
	void representative_set (store::transaction const &, nano::account const &);
	nano::public_key insert_adhoc (store::transaction const &, nano::raw_key const &);
	bool insert_watch (store::transaction const &, nano::account const &);
	void erase (store::transaction const &, nano::account const &);
	bool fetch (store::transaction const &, nano::account const &, nano::raw_key &);
	bool exists (store::transaction const &, nano::account const &);
	void destroy (store::transaction const &);
	nano::store::iterator<nano::account, nano::wallet_value> find (store::transaction const &, nano::account const &);
	nano::store::iterator<nano::account, nano::wallet_value> begin (store::transaction const &, nano::account const &);
	nano::store::iterator<nano::account, nano::wallet_value> begin (store::transaction const &);
	nano::store::iterator<nano::account, nano::wallet_value> end ();
	void derive_key (nano::raw_key &, store::transaction const &, std::string const &);
	void serialize_json (store::transaction const &, std::string &);
	void write_backup (store::transaction const &, std::filesystem::path const &);
	bool move (store::transaction const &, nano::wallet_store &, std::vector<nano::public_key> const &);
	bool import (store::transaction const &, nano::wallet_store &);
	bool work_get (store::transaction const &, nano::public_key const &, uint64_t &);
	void work_put (store::transaction const &, nano::public_key const &, uint64_t);
	unsigned version (store::transaction const &);
	static unsigned const version_4 = 4;
	static unsigned constexpr version_current = version_4;
	static int const special_count;
	nano::kdf & kdf;
	unsigned fanout;

private:
	rsnano::LmdbWalletStoreHandle * rust_handle;
};

// A wallet is a set of account keys encrypted by a common encryption key
class wallet final : public std::enable_shared_from_this<nano::wallet>
{
public:
	class representatives_lock
	{
	public:
		representatives_lock (rsnano::RepresentativesLockHandle * handle);
		representatives_lock (representatives_lock const &) = delete;
		~representatives_lock ();
		size_t size () const;
		void insert (nano::public_key const & rep);
		std::unordered_set<nano::account> get_all ();
		void set (std::unordered_set<nano::account> reps);

		rsnano::RepresentativesLockHandle * handle;
	};
	class representatives_mutex
	{
	public:
		representatives_mutex (rsnano::WalletHandle * handle);
		representatives_mutex (representatives_mutex const &) = delete;
		representatives_lock lock ();
		rsnano::WalletHandle * handle;
	};
	wallet (bool &, store::transaction &, nano::wallets &, std::string const &);
	wallet (bool &, store::transaction &, nano::wallets &, std::string const &, std::string const &);
	wallet (wallet const &) = delete;
	~wallet ();
	std::shared_ptr<nano::block> change_action (nano::account const &, nano::account const &, uint64_t = 0, bool = true);
	std::shared_ptr<nano::block> receive_action (nano::block_hash const &, nano::account const &, nano::uint128_union const &, nano::account const &, uint64_t = 0, bool = true);
	std::shared_ptr<nano::block> send_action (nano::account const &, nano::account const &, nano::uint128_t const &, uint64_t = 0, bool = true, boost::optional<std::string> = {});
	bool action_complete (std::shared_ptr<nano::block> const &, nano::account const &, bool const, nano::block_details const &);
	void enter_initial_password ();
	bool enter_password (store::transaction const &, std::string const &);
	nano::public_key insert_adhoc (nano::raw_key const &, bool = true);
	bool insert_watch (store::transaction const &, nano::public_key const &);
	nano::public_key deterministic_insert (store::transaction const &, bool = true);
	nano::public_key deterministic_insert (uint32_t, bool = true);
	nano::public_key deterministic_insert (bool = true);
	bool exists (nano::public_key const &);
	bool import (std::string const &, std::string const &);
	void serialize (std::string &);
	bool change_sync (nano::account const &, nano::account const &);
	void change_async (nano::account const &, nano::account const &, std::function<void (std::shared_ptr<nano::block> const &)> const &, uint64_t = 0, bool = true);
	bool receive_sync (std::shared_ptr<nano::block> const &, nano::account const &, nano::uint128_t const &);
	void receive_async (nano::block_hash const &, nano::account const &, nano::uint128_t const &, nano::account const &, std::function<void (std::shared_ptr<nano::block> const &)> const &, uint64_t = 0, bool = true);
	nano::block_hash send_sync (nano::account const &, nano::account const &, nano::uint128_t const &);
	void send_async (nano::account const &, nano::account const &, nano::uint128_t const &, std::function<void (std::shared_ptr<nano::block> const &)> const &, uint64_t = 0, bool = true, boost::optional<std::string> = {});
	void work_cache_blocking (nano::account const &, nano::root const &);
	void work_update (store::transaction const &, nano::account const &, nano::root const &, uint64_t);
	// Schedule work generation after a few seconds
	void work_ensure (nano::account const &, nano::root const &);
	bool search_receivable (store::transaction const &);
	uint32_t deterministic_check (store::transaction const & transaction_a, uint32_t index);
	/** Changes the wallet seed and returns the first account */
	nano::public_key change_seed (store::transaction const & transaction_a, nano::raw_key const & prv_a, uint32_t count = 0);
	void deterministic_restore (store::transaction const & transaction_a);
	bool live ();

	nano::wallet_store store;
	nano::wallets & wallets;
	nano::wallet_action_thread & wallet_actions;
	nano::wallet_representatives & representatives;
	nano::node & node;
	nano::store::lmdb::env & env;
	rsnano::WalletHandle * handle;
	representatives_mutex representatives_mutex;
};

class wallet_action_thread
{
public:
	wallet_action_thread ();

	void start ();
	void stop ();
	void queue_wallet_action (nano::uint128_t const &, std::shared_ptr<nano::wallet> const &, std::function<void (nano::wallet &)>);
	nano::lock_guard<nano::mutex> lock ();
	size_t size ();

	std::function<void (bool)> observer;

private:
	void do_wallet_actions ();

	nano::mutex action_mutex;
	std::atomic<bool> stopped;
	nano::condition_variable condition;
	std::multimap<nano::uint128_t, std::pair<std::shared_ptr<nano::wallet>, std::function<void (nano::wallet &)>>, std::greater<nano::uint128_t>> actions;
	std::thread thread;
};

class wallet_representatives
{
public:
	wallet_representatives (nano::node & node_a) :
		node{ node_a }
	{
	}
	uint64_t voting{ 0 }; // Number of representatives with at least the configured minimum voting weight
	bool half_principal{ false }; // has representatives with at least 50% of principal representative requirements
	std::unordered_set<nano::account> accounts; // Representatives with at least the configured minimum voting weight
	bool have_half_rep () const
	{
		return half_principal;
	}
	bool exists (nano::account const & rep_a) const
	{
		return accounts.count (rep_a) > 0;
	}
	void clear ()
	{
		voting = 0;
		half_principal = false;
		accounts.clear ();
	}
	bool check_rep (nano::account const &, nano::uint128_t const &, bool const = true);

	mutable nano::mutex reps_cache_mutex;
	nano::node & node;
};

enum class [[nodiscard]] wallets_error{
	none = 0,
	generic,
	wallet_not_found,
	wallet_locked,
	account_not_found,
};

/**
 * The wallets set is all the wallets a node controls.
 * A node may contain multiple wallets independently encrypted and operated.
 */
class wallets final
{
public:
	class wallets_mutex_lock
	{
	public:
		wallets_mutex_lock (rsnano::WalletsMutexLockHandle * handle);
		wallets_mutex_lock (wallets_mutex_lock &&);
		wallets_mutex_lock (wallets_mutex_lock const &) = delete;
		~wallets_mutex_lock ();
		std::shared_ptr<nano::wallet> find (nano::wallet_id const & wallet_id);
		rsnano::WalletsMutexLockHandle * handle;
	};

	class wallets_mutex
	{
	public:
		wallets_mutex (rsnano::LmdbWalletsHandle * handle);
		wallets_mutex (wallets_mutex_lock const &) = delete;
		boost::optional<wallets_mutex_lock> try_lock ();
		wallets_mutex_lock lock ();
		rsnano::LmdbWalletsHandle * handle;
	};

	wallets (bool, nano::node &);
	~wallets ();
	std::shared_ptr<nano::wallet> open (nano::wallet_id const &);
	std::shared_ptr<nano::wallet> create (nano::wallet_id const &);
	size_t wallet_count () const;
	size_t representatives_count (nano::wallet_id const & id) const;
	nano::account get_representative (store::transaction const &, nano::wallet_id const & id) const;
	void set_representative (nano::wallet_id const & wallet_id, nano::account const & rep);
	void get_seed (nano::raw_key & prv_a, store::transaction const & transaction_a, nano::wallet_id const & id) const;
	nano::public_key change_seed (nano::wallet_id const & wallet_id, store::transaction const & transaction_a, nano::raw_key const & prv_a, uint32_t count = 0);
	bool ensure_wallet_is_unlocked (nano::wallet_id const & wallet_id, std::string const & password_a);
	bool import (nano::wallet_id const & wallet_id, std::string const & json_a, std::string const & password_a);
	std::vector<std::pair<nano::account, nano::raw_key>> decrypt (store::transaction const & txn, nano::wallet_id const & wallet_id) const;
	nano::wallets_error fetch (nano::wallet_id const & wallet_id, nano::account const & pub, nano::raw_key & prv);
	std::vector<nano::wallet_id> get_wallet_ids () const;
	std::vector<nano::account> get_accounts (nano::wallet_id const & wallet_id);
	std::vector<nano::account> get_accounts (size_t max_results);
	uint64_t work_get (nano::wallet_id const & wallet_id, nano::account const & account);
	nano::wallets_error remove_account (nano::wallet_id const & wallet_id, nano::account & account_id);
	bool move_accounts (nano::wallet_id const & source_id, nano::wallet_id const & target_id, std::vector<nano::public_key> const & accounts);
	bool wallet_exists (nano::wallet_id const & id) const;
	nano::wallet_id first_wallet_id () const;
	nano::public_key insert_adhoc (nano::wallet_id const & id, nano::raw_key const & key_a, bool generate_work_a = true);
	void set_password (nano::wallet_id const & wallet_id, nano::raw_key const & password);
	void password (nano::wallet_id const & wallet_id, nano::raw_key & password_a) const;
	bool enter_password (nano::wallet_id const & id, store::transaction const & transaction_a, std::string const & password_a);
	void enter_initial_password (nano::wallet_id const & wallet_id);
	bool valid_password (nano::wallet_id const & wallet_id, store::transaction const &);
	bool attempt_password (nano::wallet_id const & wallet_id, store::transaction const &, std::string const &);
	void rekey (nano::wallet_id const wallet_id, std::string const &);
	nano::public_key deterministic_insert (nano::wallet_id const & wallet_id);
	void deterministic_restore (nano::wallet_id const & wallet_id, store::transaction const & transaction_a);
	void backup (std::filesystem::path const & backup_path);
	void work_cache_blocking (nano::wallet_id const & wallet_id, nano::account const & account_a, nano::root const & root_a);
	std::shared_ptr<nano::block> send_action (nano::wallet_id const & wallet_id, nano::account const &, nano::account const &, nano::uint128_t const &, uint64_t = 0, bool = true, boost::optional<std::string> = {});
	std::shared_ptr<nano::block> change_action (nano::wallet_id const & wallet_id, nano::account const &, nano::account const &, uint64_t = 0, bool = true);
	std::shared_ptr<nano::block> receive_action (nano::wallet_id const & wallet_id, nano::block_hash const &, nano::account const &, nano::uint128_union const &, nano::account const &, uint64_t = 0, bool = true);
	nano::block_hash send_sync (nano::wallet_id const & wallet_id, nano::account const &, nano::account const &, nano::uint128_t const &);
	bool receive_sync (nano::wallet_id const & wallet_id, std::shared_ptr<nano::block> const &, nano::account const &, nano::uint128_t const &);
	void send_async (nano::wallet_id const & wallet_id, nano::account const &, nano::account const &, nano::uint128_t const &, std::function<void (std::shared_ptr<nano::block> const &)> const &, uint64_t = 0, bool = true, boost::optional<std::string> = {});
	bool change_sync (nano::wallet_id const & wallet_id, nano::account const &, nano::account const &);
	nano::wallets_error change_async (nano::wallet_id const & wallet_id, nano::account const &, nano::account const &, std::function<void (std::shared_ptr<nano::block> const &)> const &, uint64_t = 0, bool = true);
	void serialize (nano::wallet_id const & wallet_id, std::string &);
	bool search_receivable (nano::wallet_id const &);
	void search_receivable_all ();
	void destroy (nano::wallet_id const &);
	void reload ();
	void foreach_representative (std::function<void (nano::public_key const &, nano::raw_key const &)> const &);
	bool exists (nano::account const & account_a);
	bool exists (store::transaction const &, nano::account const &);
	void clear_send_ids (store::transaction const &);
	size_t voting_reps_count () const;
	bool have_half_rep () const;
	bool rep_exists (nano::account const & rep) const;
	bool should_republish_vote (nano::account const & voting_account) const;
	void compute_reps ();
	void ongoing_compute_reps ();
	std::vector<nano::wallet_id> get_wallet_ids (store::transaction const & transaction_a);
	std::unordered_map<nano::wallet_id, std::shared_ptr<nano::wallet>> get_wallets ();
	nano::block_hash get_block_hash (bool & error_a, store::transaction const & transaction_a, std::string const & id_a);
	bool set_block_hash (store::transaction const & transaction_a, std::string const & id_a, nano::block_hash const & hash);
	nano::network_params & network_params;
	std::unordered_map<nano::wallet_id, std::shared_ptr<nano::wallet>> items;
	nano::wallet_action_thread wallet_actions;
	nano::locked<std::unordered_map<nano::account, nano::root>> delayed_work;
	nano::kdf kdf;
	nano::node & node;
	nano::store::lmdb::env & env;
	static nano::uint128_t const generate_priority;
	static nano::uint128_t const high_priority;
	/** Start read-write transaction */
	std::unique_ptr<store::write_transaction> tx_begin_write ();

	/** Start read-only transaction */
	std::unique_ptr<store::read_transaction> tx_begin_read ();

	nano::wallet_representatives representatives;
	rsnano::LmdbWalletsHandle * rust_handle;
	mutable wallets_mutex mutex;
};

std::unique_ptr<container_info_component> collect_container_info (wallets & wallets, std::string const & name);

class wallets_store
{
public:
	virtual ~wallets_store () = default;
	virtual bool init_error () const = 0;
};
class mdb_wallets_store final : public wallets_store
{
public:
	mdb_wallets_store (std::filesystem::path const &, nano::lmdb_config const & lmdb_config_a = nano::lmdb_config{});
	nano::store::lmdb::env environment;
	bool init_error () const override;
	bool error{ false };
};
}
