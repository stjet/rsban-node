#pragma once

#include "nano/lib/rsnano.hpp"

#include <nano/lib/lmdbconfig.hpp>
#include <nano/lib/locks.hpp>
#include <nano/lib/work.hpp>
#include <nano/secure/common.hpp>
#include <nano/store/component.hpp>
#include <nano/store/lmdb/lmdb.hpp>
#include <nano/store/lmdb/wallet_value.hpp>

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
	wallet_store (rsnano::LmdbWalletStoreHandle * handle);
	~wallet_store ();
	wallet_store (wallet_store const &) = delete;
	void password (nano::raw_key & password_a) const;
	void set_password (nano::raw_key const & password_a);
	nano::uint256_union check (store::transaction const &);
	bool rekey (store::transaction const &, std::string const &);
	bool valid_password (store::transaction const &);
	bool attempt_password (store::transaction const &, std::string const &);
	void wallet_key (nano::raw_key &, store::transaction const &);
	void seed (nano::raw_key &, store::transaction const &);
	void seed_set (store::transaction const &, nano::raw_key const &);
	nano::public_key deterministic_insert (store::transaction const &);
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
	nano::store::iterator<nano::account, nano::wallet_value> find (store::transaction const &, nano::account const &);
	nano::store::iterator<nano::account, nano::wallet_value> begin (store::transaction const &);
	nano::store::iterator<nano::account, nano::wallet_value> end ();
	void derive_key (nano::raw_key &, store::transaction const &, std::string const &);
	void serialize_json (store::transaction const &, std::string &);
	bool move (store::transaction const &, nano::wallet_store &, std::vector<nano::public_key> const &);
	static unsigned const version_4 = 4;
	static unsigned constexpr version_current = version_4;
	static int const special_count;

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
	wallet (rsnano::WalletHandle * handle);
	wallet (wallet const &) = delete;
	~wallet ();
	size_t representatives_count () const;
	rsnano::WalletHandle * handle;
	nano::wallet_store store;
	mutable representatives_mutex representatives_mutex;
};

class wallet_representatives_lock
{
public:
	wallet_representatives_lock (rsnano::WalletRepresentativesLock * handle);
	wallet_representatives_lock (wallet_representatives_lock const &) = delete;
	wallet_representatives_lock (wallet_representatives_lock &&);
	~wallet_representatives_lock ();
	bool have_half_rep () const;
	uint64_t voting_reps () const;
	bool exists (nano::account const & rep_a) const;
	void clear ();
	bool check_rep (nano::account const &, nano::uint128_t const &);
	rsnano::WalletRepresentativesLock * handle;
};

enum class [[nodiscard]] wallets_error
{
	none = 0,
	generic,
	wallet_not_found,
	wallet_locked,
	account_not_found,
	invalid_password,
	bad_public_key,
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
		std::unordered_map<nano::wallet_id, std::shared_ptr<nano::wallet>> get_all ();
		size_t size () const;

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

	wallets (nano::node &);
	wallets (rsnano::LmdbWalletsHandle * handle);
	~wallets ();

	void create (nano::wallet_id const &);
	size_t wallet_count () const;
	size_t representatives_count (nano::wallet_id const & id) const;
	nano::key_type key_type (nano::wallet_id const & wallet_id, nano::account const & account);

	nano::wallets_error get_seed (nano::wallet_id const & wallet_id, nano::raw_key & prv_a) const;
	/** Changes the wallet seed and returns the first account */
	nano::wallets_error change_seed (nano::wallet_id const & wallet_id, nano::raw_key const & prv_a, uint32_t count, nano::public_key & first_account, uint32_t & restored_count);

	bool import_replace (nano::wallet_id const & wallet_id, std::string const & json_a, std::string const & password_a);
	bool import (nano::wallet_id const & wallet_id, std::string const & json_a);
	nano::wallets_error serialize (nano::wallet_id const & wallet_id, std::string &);
	nano::wallets_error fetch (nano::wallet_id const & wallet_id, nano::account const & pub, nano::raw_key & prv);
	nano::wallets_error decrypt (nano::wallet_id const & wallet_id, std::vector<std::pair<nano::account, nano::raw_key>> & accounts) const;

	std::vector<nano::wallet_id> get_wallet_ids () const;
	bool wallet_exists (nano::wallet_id const & id) const;
	nano::wallet_id first_wallet_id () const;

	nano::wallets_error get_accounts (nano::wallet_id const & wallet_id, std::vector<nano::account> & accounts);
	bool move_accounts (nano::wallet_id const & source_id, nano::wallet_id const & target_id, std::vector<nano::public_key> const & accounts);
	nano::wallets_error remove_account (nano::wallet_id const & wallet_id, nano::account const & account_id);

	nano::wallets_error get_representative (nano::wallet_id const & id, nano::account & representative);
	nano::wallets_error set_representative (nano::wallet_id const & wallet_id, nano::account const & rep, bool update_existing_accounts = false);

	uint64_t work_get (nano::wallet_id const & wallet_id, nano::account const & account);
	nano::wallets_error work_get (nano::wallet_id const & wallet_id, nano::account const & account, uint64_t & work);
	nano::wallets_error work_set (nano::wallet_id const & wallet_id, nano::account const & account, uint64_t work);

	void set_password (nano::wallet_id const & wallet_id, nano::raw_key const & password);
	void password (nano::wallet_id const & wallet_id, nano::raw_key & password_a) const;
	nano::wallets_error enter_password (nano::wallet_id const & id, std::string const & password_a);
	void enter_initial_password (nano::wallet_id const & wallet_id);
	nano::wallets_error valid_password (nano::wallet_id const & wallet_id, bool & valid);
	nano::wallets_error attempt_password (nano::wallet_id const & wallet_id, std::string const &);
	nano::wallets_error rekey (nano::wallet_id const wallet_id, std::string const &);
	nano::wallets_error lock (nano::wallet_id const & wallet_id);
	bool ensure_wallet_is_unlocked (nano::wallet_id const & wallet_id, std::string const & password_a);

	nano::wallets_error insert_adhoc (nano::wallet_id const & id, nano::raw_key const & key_a, bool generate_work_a = true);
	nano::wallets_error insert_adhoc (nano::wallet_id const & id, nano::raw_key const & key_a, bool generate_work_a, nano::public_key & account);
	nano::wallets_error insert_watch (nano::wallet_id const & wallet_id, std::vector<nano::public_key> const & accounts);
	nano::wallets_error deterministic_insert (nano::wallet_id const & wallet_id, uint32_t const index, bool generate_work_a, nano::account & account);
	nano::wallets_error deterministic_insert (nano::wallet_id const & wallet_id, bool generate_work_a, nano::account & account);
	nano::wallets_error deterministic_index_get (nano::wallet_id const & wallet_id, uint32_t & index);

	void work_cache_blocking (nano::wallet_id const & wallet_id, nano::account const & account_a, nano::root const & root_a);

	std::shared_ptr<nano::block> send_action (nano::wallet_id const & wallet_id, nano::account const &, nano::account const &, nano::uint128_t const &, uint64_t = 0, bool = true, boost::optional<std::string> = {});
	std::shared_ptr<nano::block> change_action (nano::wallet_id const & wallet_id, nano::account const &, nano::account const &, uint64_t = 0, bool = true);
	std::shared_ptr<nano::block> receive_action (nano::wallet_id const & wallet_id, nano::block_hash const &, nano::account const &, nano::uint128_union const &, nano::account const &, uint64_t = 0, bool = true);
	nano::block_hash send_sync (nano::wallet_id const & wallet_id, nano::account const &, nano::account const &, nano::uint128_t const &);
	bool receive_sync (nano::wallet_id const & wallet_id, std::shared_ptr<nano::block> const &, nano::account const &, nano::uint128_t const &);
	bool change_sync (nano::wallet_id const & wallet_id, nano::account const &, nano::account const &);
	nano::wallets_error receive_async (nano::wallet_id const & wallet_id, nano::block_hash const &, nano::account const &, nano::uint128_t const &, nano::account const &, std::function<void (std::shared_ptr<nano::block> const &)> const &, uint64_t = 0, bool = true);
	nano::wallets_error change_async (nano::wallet_id const & wallet_id, nano::account const &, nano::account const &, std::function<void (std::shared_ptr<nano::block> const &)> const &, uint64_t = 0, bool = true);
	nano::wallets_error send_async (nano::wallet_id const & wallet_id, nano::account const & source_a, nano::account const & account_a, nano::uint128_t const & amount_a, std::function<void (std::shared_ptr<nano::block> const &)> const & action_a, uint64_t work_a = 0, bool generate_work_a = true, boost::optional<std::string> id_a = {});

	nano::wallets_error search_receivable (nano::wallet_id const &);
	void search_receivable_all ();

	void destroy (nano::wallet_id const &);
	void reload ();
	void foreach_representative (std::function<void (nano::public_key const &, nano::raw_key const &)> const &);
	bool exists (nano::account const & account_a);
	void clear_send_ids ();
	size_t voting_reps_count () const;
	bool have_half_rep () const;
	bool rep_exists (nano::account const & rep) const;
	void compute_reps ();

public: // TODO make private
	std::shared_ptr<nano::block> receive_action (const std::shared_ptr<nano::wallet> & wallet, nano::block_hash const & send_hash_a, nano::account const & representative_a, nano::uint128_union const & amount_a, nano::account const & account_a, uint64_t work_a, bool generate_work_a);
	std::shared_ptr<nano::block> change_action (const std::shared_ptr<wallet> & wallet, nano::account const & source_a, nano::account const & representative_a, uint64_t work_a, bool generate_work_a);
	std::shared_ptr<nano::block> send_action (const std::shared_ptr<nano::wallet> & wallet, nano::account const & source_a, nano::account const & account_a, nano::uint128_t const & amount_a, uint64_t work_a, bool generate_work_a, boost::optional<std::string> id_a);
	bool change_sync (const std::shared_ptr<nano::wallet> & wallet, nano::account const & source_a, nano::account const & representative_a);
	bool receive_sync (const std::shared_ptr<nano::wallet> & wallet, std::shared_ptr<nano::block> const & block_a, nano::account const & representative_a, nano::uint128_t const & amount_a);
	void enter_initial_password (const std::shared_ptr<nano::wallet> & wallet);
	nano::root get_delayed_work (nano::account const & account);
	void stop_actions ();
	nano::wallet_representatives_lock lock_representatives () const;

	// fields
public:
	rsnano::LmdbWalletsHandle * rust_handle;
	mutable wallets_mutex mutex;
};

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
