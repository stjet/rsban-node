#include <nano/crypto_lib/random_pool.hpp>
#include <nano/lib/blocks.hpp>
#include <nano/lib/thread_runner.hpp>
#include <nano/lib/threading.hpp>
#include <nano/node/active_elections.hpp>
#include <nano/node/election.hpp>
#include <nano/secure/ledger.hpp>
#include <nano/store/lmdb/wallet_value.hpp>
#include <nano/test_common/system.hpp>
#include <nano/test_common/testutil.hpp>

#include <gtest/gtest.h>

#include <boost/filesystem.hpp>

#include <cstdint>

using namespace std::chrono_literals;
unsigned constexpr nano::wallet_store::version_current;

TEST (wallet, serialize_json_password)
{
	auto error (false);
	nano::store::lmdb::env env (error, nano::unique_path () / "wallet.ldb");
	ASSERT_FALSE (error);
	auto transaction (env.tx_begin_write ());
	nano::kdf kdf{ nano::dev::network_params.kdf_work };
	nano::wallet_store wallet1 (error, kdf, *transaction, nano::dev::genesis_key.pub, 1, "0");
	ASSERT_FALSE (error);
	nano::keypair key;
	wallet1.rekey (*transaction, "password");
	wallet1.insert_adhoc (*transaction, key.prv);
	std::string serialized;
	wallet1.serialize_json (*transaction, serialized);
	nano::wallet_store wallet2 (error, kdf, *transaction, nano::dev::genesis_key.pub, 1, "1", serialized);
	ASSERT_FALSE (error);
	ASSERT_FALSE (wallet2.valid_password (*transaction));
	ASSERT_FALSE (wallet2.attempt_password (*transaction, "password"));
	ASSERT_TRUE (wallet2.valid_password (*transaction));
	nano::raw_key password1;
	nano::raw_key password2;
	wallet1.wallet_key (password1, *transaction);
	wallet2.wallet_key (password2, *transaction);
	ASSERT_EQ (password1, password2);
	ASSERT_EQ (wallet1.salt (*transaction), wallet2.salt (*transaction));
	ASSERT_EQ (wallet1.check (*transaction), wallet2.check (*transaction));
	ASSERT_EQ (wallet1.representative (*transaction), wallet2.representative (*transaction));
	ASSERT_TRUE (wallet2.exists (*transaction, key.pub));
	nano::raw_key prv;
	wallet2.fetch (*transaction, key.pub, prv);
	ASSERT_EQ (key.prv, prv);
}

TEST (wallet_store, move)
{
	auto error (false);
	nano::store::lmdb::env env (error, nano::unique_path () / "wallet.ldb");
	ASSERT_FALSE (error);
	auto transaction (env.tx_begin_write ());
	nano::kdf kdf{ nano::dev::network_params.kdf_work };
	nano::wallet_store wallet1 (error, kdf, *transaction, nano::dev::genesis_key.pub, 1, "0");
	ASSERT_FALSE (error);
	nano::keypair key1;
	wallet1.insert_adhoc (*transaction, key1.prv);
	nano::wallet_store wallet2 (error, kdf, *transaction, nano::dev::genesis_key.pub, 1, "1");
	ASSERT_FALSE (error);
	nano::keypair key2;
	wallet2.insert_adhoc (*transaction, key2.prv);
	ASSERT_FALSE (wallet1.exists (*transaction, key2.pub));
	ASSERT_TRUE (wallet2.exists (*transaction, key2.pub));
	std::vector<nano::public_key> keys;
	keys.push_back (key2.pub);
	ASSERT_FALSE (wallet1.move (*transaction, wallet2, keys));
	ASSERT_TRUE (wallet1.exists (*transaction, key2.pub));
	ASSERT_FALSE (wallet2.exists (*transaction, key2.pub));
}

TEST (wallet_store, import)
{
	nano::test::system system (2);
	auto node1 = system.nodes[0];
	auto node2 = system.nodes[1];
	auto wallet_id1 = node1->wallets.first_wallet_id ();
	auto wallet_id2 = node2->wallets.first_wallet_id ();
	nano::keypair key1;
	(void)node1->wallets.insert_adhoc (wallet_id1, key1.prv);
	std::string json;
	ASSERT_EQ (nano::wallets_error::none, node1->wallets.serialize (wallet_id1, json));
	ASSERT_FALSE (node2->wallets.exists (key1.pub));
	auto error (node2->wallets.import_replace (wallet_id2, json, ""));
	ASSERT_FALSE (error);
	ASSERT_TRUE (node2->wallets.exists (key1.pub));
}

TEST (wallet_store, fail_import_bad_password)
{
	nano::test::system system (2);
	auto node1 = system.nodes[0];
	auto node2 = system.nodes[1];
	auto wallet_id1 = node1->wallets.first_wallet_id ();
	auto wallet_id2 = node2->wallets.first_wallet_id ();
	nano::keypair key1;
	(void)node1->wallets.insert_adhoc (wallet_id1, key1.prv);
	std::string json;
	ASSERT_EQ (nano::wallets_error::none, node1->wallets.serialize (wallet_id1, json));
	ASSERT_FALSE (node2->wallets.exists (key1.pub));
	auto error (node2->wallets.import_replace (wallet_id2, json, "1"));
	ASSERT_TRUE (error);
}

TEST (wallet_store, fail_import_corrupt)
{
	nano::test::system system (1);
	auto node1 = system.nodes[0];
	auto wallet_id1 = node1->wallets.first_wallet_id ();
	std::string json;
	auto error (node1->wallets.import_replace (wallet_id1, json, "1"));
	ASSERT_TRUE (error);
}

// Test work is precached when a key is inserted
TEST (wallet, work)
{
	nano::test::system system (1);
	auto node1 (system.nodes[0]);
	auto wallet_id1 = node1->wallets.first_wallet_id ();
	(void)node1->wallets.insert_adhoc (wallet_id1, nano::dev::genesis_key.prv);
	(void)node1->wallets.insert_adhoc (wallet_id1, nano::dev::genesis_key.prv);
	auto done (false);
	system.deadline_set (20s);
	while (!done)
	{
		auto work = node1->wallets.work_get (wallet_id1, nano::dev::genesis_key.pub);
		done = nano::dev::network_params.work.difficulty (nano::dev::genesis->work_version (), nano::dev::genesis->hash (), work) >= system.nodes[0]->default_difficulty (nano::dev::genesis->work_version ());
		ASSERT_NO_ERROR (system.poll ());
	}
}

TEST (wallet, work_generate)
{
	nano::test::system system (1);
	auto & node1 (*system.nodes[0]);
	auto wallet_id = node1.wallets.first_wallet_id ();
	nano::uint128_t amount1 (node1.balance (nano::dev::genesis_key.pub));
	uint64_t work1;
	(void)node1.wallets.insert_adhoc (wallet_id, nano::dev::genesis_key.prv);
	auto account1{ system.account (0) };
	nano::keypair key;
	auto block (node1.wallets.send_action (wallet_id, nano::dev::genesis_key.pub, key.pub, 100));
	ASSERT_TIMELY (10s, node1.ledger.any ().account_balance (*node1.ledger.store.tx_begin_read (), nano::dev::genesis_key.pub) != amount1);
	system.deadline_set (10s);
	auto again (true);
	while (again)
	{
		ASSERT_NO_ERROR (system.poll ());
		auto block_transaction (node1.store.tx_begin_read ());
		auto work1 = node1.wallets.work_get (wallet_id, account1);
		again = nano::dev::network_params.work.difficulty (block->work_version (), node1.ledger.latest_root (*block_transaction, account1), work1) < node1.default_difficulty (block->work_version ());
	}
}

TEST (wallet, work_cache_delayed)
{
	nano::test::system system (1);
	auto & node1 (*system.nodes[0]);
	auto wallet_id = node1.wallets.first_wallet_id ();
	uint64_t work1;
	(void)node1.wallets.insert_adhoc (wallet_id, nano::dev::genesis_key.prv);
	auto account1{ system.account (0) };
	nano::keypair key;
	auto block1 (node1.wallets.send_action (wallet_id, nano::dev::genesis_key.pub, key.pub, 100));
	ASSERT_EQ (block1->hash (), node1.latest (nano::dev::genesis_key.pub));
	auto block2 (node1.wallets.send_action (wallet_id, nano::dev::genesis_key.pub, key.pub, 100));
	ASSERT_EQ (block2->hash (), node1.latest (nano::dev::genesis_key.pub));
	ASSERT_EQ (block2->hash (), node1.wallets.get_delayed_work (nano::dev::genesis_key.pub).as_block_hash ());
	auto threshold (node1.default_difficulty (nano::work_version::work_1));
	auto again (true);
	system.deadline_set (10s);
	while (again)
	{
		ASSERT_NO_ERROR (system.poll ());
		work1 = node1.wallets.work_get (wallet_id, account1);
		again = nano::dev::network_params.work.difficulty (nano::work_version::work_1, block2->hash (), work1) < threshold;
	}
	ASSERT_GE (nano::dev::network_params.work.difficulty (nano::work_version::work_1, block2->hash (), work1), threshold);
}

TEST (wallet, insert_locked)
{
	nano::test::system system (1);
	auto & node1 (*system.nodes[0]);
	auto wallet_id{ node1.wallets.first_wallet_id () };
	{
		ASSERT_EQ (nano::wallets_error::none, node1.wallets.rekey (wallet_id, "1"));
		{
			bool valid = false;
			(void)node1.wallets.valid_password (wallet_id, valid);
			ASSERT_TRUE (valid);
		}
		ASSERT_EQ (nano::wallets_error::invalid_password, node1.wallets.enter_password (wallet_id, ""));
	}
	bool valid = false;
	(void)node1.wallets.valid_password (wallet_id, valid);
	ASSERT_FALSE (valid);
	nano::account account;
	(void)node1.wallets.insert_adhoc (wallet_id, nano::keypair ().prv, true, account);
	ASSERT_TRUE (account.is_zero ());
}

TEST (wallet, deterministic_keys)
{
	bool init;
	nano::store::lmdb::env env (init, nano::unique_path () / "wallet.ldb");
	ASSERT_FALSE (init);
	auto transaction (env.tx_begin_write ());
	nano::kdf kdf{ nano::dev::network_params.kdf_work };
	nano::wallet_store wallet (init, kdf, *transaction, nano::dev::genesis_key.pub, 1, "0");
	auto key1 = wallet.deterministic_key (*transaction, 0);
	auto key2 = wallet.deterministic_key (*transaction, 0);
	ASSERT_EQ (key1, key2);
	auto key3 = wallet.deterministic_key (*transaction, 1);
	ASSERT_NE (key1, key3);
	ASSERT_EQ (0, wallet.deterministic_index_get (*transaction));
	wallet.deterministic_index_set (*transaction, 1);
	ASSERT_EQ (1, wallet.deterministic_index_get (*transaction));
	auto key4 (wallet.deterministic_insert (*transaction));
	nano::raw_key key5;
	ASSERT_FALSE (wallet.fetch (*transaction, key4, key5));
	ASSERT_EQ (key3, key5);
	ASSERT_EQ (2, wallet.deterministic_index_get (*transaction));
	wallet.deterministic_index_set (*transaction, 1);
	ASSERT_EQ (1, wallet.deterministic_index_get (*transaction));
	wallet.erase (*transaction, key4);
	ASSERT_FALSE (wallet.exists (*transaction, key4));
	auto key8 (wallet.deterministic_insert (*transaction));
	ASSERT_EQ (key4, key8);
	auto key6 (wallet.deterministic_insert (*transaction));
	nano::raw_key key7;
	ASSERT_FALSE (wallet.fetch (*transaction, key6, key7));
	ASSERT_NE (key5, key7);
	ASSERT_EQ (3, wallet.deterministic_index_get (*transaction));
	nano::keypair key9;
	ASSERT_EQ (key9.pub, wallet.insert_adhoc (*transaction, key9.prv));
	ASSERT_TRUE (wallet.exists (*transaction, key9.pub));
	wallet.deterministic_clear (*transaction);
	ASSERT_EQ (0, wallet.deterministic_index_get (*transaction));
	ASSERT_FALSE (wallet.exists (*transaction, key4));
	ASSERT_FALSE (wallet.exists (*transaction, key6));
	ASSERT_FALSE (wallet.exists (*transaction, key8));
	ASSERT_TRUE (wallet.exists (*transaction, key9.pub));
}

TEST (wallet, reseed)
{
	bool init;
	nano::store::lmdb::env env (init, nano::unique_path () / "wallet.ldb");
	ASSERT_FALSE (init);
	auto transaction (env.tx_begin_write ());
	nano::kdf kdf{ nano::dev::network_params.kdf_work };
	nano::wallet_store wallet (init, kdf, *transaction, nano::dev::genesis_key.pub, 1, "0");
	nano::raw_key seed1;
	seed1 = 1;
	nano::raw_key seed2;
	seed2 = 2;
	wallet.seed_set (*transaction, seed1);
	nano::raw_key seed3;
	wallet.seed (seed3, *transaction);
	ASSERT_EQ (seed1, seed3);
	auto key1 (wallet.deterministic_insert (*transaction));
	ASSERT_EQ (1, wallet.deterministic_index_get (*transaction));
	wallet.seed_set (*transaction, seed2);
	ASSERT_EQ (0, wallet.deterministic_index_get (*transaction));
	nano::raw_key seed4;
	wallet.seed (seed4, *transaction);
	ASSERT_EQ (seed2, seed4);
	auto key2 (wallet.deterministic_insert (*transaction));
	ASSERT_NE (key1, key2);
	wallet.seed_set (*transaction, seed1);
	nano::raw_key seed5;
	wallet.seed (seed5, *transaction);
	ASSERT_EQ (seed1, seed5);
	auto key3 (wallet.deterministic_insert (*transaction));
	ASSERT_EQ (key1, key3);
}

TEST (wallet, insert_deterministic_locked)
{
	nano::test::system system (1);
	auto & node1 (*system.nodes[0]);
	auto wallet_id{ node1.wallets.first_wallet_id () };
	ASSERT_EQ (nano::wallets_error::none, node1.wallets.rekey (wallet_id, "1"));
	{
		bool valid = false;
		(void)node1.wallets.valid_password (wallet_id, valid);
		ASSERT_TRUE (valid);
		ASSERT_EQ (nano::wallets_error::invalid_password, node1.wallets.enter_password (wallet_id, ""));
		(void)node1.wallets.valid_password (wallet_id, valid);
		ASSERT_FALSE (valid);
	}
	nano::public_key key;
	ASSERT_EQ (nano::wallets_error::wallet_locked, node1.wallets.deterministic_insert (wallet_id, true, key));
	ASSERT_TRUE (key.is_zero ());
}

TEST (wallet, no_work)
{
	nano::test::system system (1);
	auto & node1 (*system.nodes[0]);
	auto wallet_id = node1.wallets.first_wallet_id ();
	(void)node1.wallets.insert_adhoc (wallet_id, nano::dev::genesis_key.prv, false);
	nano::keypair key2;
	auto block (node1.wallets.send_action (wallet_id, nano::dev::genesis_key.pub, key2.pub, std::numeric_limits<nano::uint128_t>::max (), false));
	ASSERT_NE (nullptr, block);
	ASSERT_NE (0, block->block_work ());
	ASSERT_GE (nano::dev::network_params.work.difficulty (*block), nano::dev::network_params.work.threshold (block->work_version (), block->sideband ().details ()));
	auto cached_work = node1.wallets.work_get (wallet_id, nano::dev::genesis_key.pub);
	ASSERT_EQ (0, cached_work);
}

TEST (wallet, send_race)
{
	nano::test::system system (1);
	auto & node (*system.nodes[0]);
	auto wallet_id = node.wallets.first_wallet_id ();

	(void)node.wallets.insert_adhoc (wallet_id, nano::dev::genesis_key.prv);
	nano::keypair key2;
	for (auto i (1); i < 60; ++i)
	{
		ASSERT_NE (nullptr, node.wallets.send_action (wallet_id, nano::dev::genesis_key.pub, key2.pub, nano::Gxrb_ratio));
		ASSERT_EQ (nano::dev::constants.genesis_amount - nano::Gxrb_ratio * i, node.balance (nano::dev::genesis_key.pub));
	}
}

TEST (wallet, password_race)
{
	nano::test::system system (1);
	auto & node1 (*system.nodes[0]);
	auto wallet_id = node1.wallets.first_wallet_id ();
	nano::thread_runner runner (system.async_rt.io_ctx, node1.config->io_threads);
	std::thread thread ([&wallet_id, &node1] () {
		for (int i = 0; i < 100; i++)
		{
			ASSERT_EQ (nano::wallets_error::none, node1.wallets.rekey (wallet_id, std::to_string (i)));
		}
	});
	for (int i = 0; i < 100; i++)
	{
		// Password should always be valid, the rekey operation should be atomic.
		bool ok = false;
		ASSERT_EQ (nano::wallets_error::none, node1.wallets.valid_password (wallet_id, ok));
		EXPECT_TRUE (ok);
		if (!ok)
		{
			break;
		}
	}
	thread.join ();
	system.stop ();
	runner.join ();
}

TEST (wallet, password_race_corrupt_seed)
{
	nano::test::system system (1);
	auto & node1 (*system.nodes[0]);
	auto wallet_id = node1.wallets.first_wallet_id ();
	nano::thread_runner runner (system.async_rt.io_ctx, system.nodes[0]->config->io_threads);
	nano::raw_key seed;
	ASSERT_EQ (nano::wallets_error::none, node1.wallets.rekey (wallet_id, "4567"));
	ASSERT_EQ (nano::wallets_error::none, node1.wallets.get_seed (wallet_id, seed));
	ASSERT_EQ (nano::wallets_error::none, node1.wallets.attempt_password (wallet_id, "4567"));
	std::vector<std::thread> threads;
	for (int i = 0; i < 100; i++)
	{
		threads.emplace_back ([&node1, &wallet_id] () {
			for (int i = 0; i < 10; i++)
			{
				(void)node1.wallets.rekey (wallet_id, "0000");
			}
		});
		threads.emplace_back ([&node1, &wallet_id] () {
			for (int i = 0; i < 10; i++)
			{
				(void)node1.wallets.rekey (wallet_id, "1234");
			}
		});
		threads.emplace_back ([&node1, &wallet_id] () {
			for (int i = 0; i < 10; i++)
			{
				(void)node1.wallets.attempt_password (wallet_id, "1234");
			}
		});
	}
	for (auto & thread : threads)
	{
		thread.join ();
	}
	system.stop ();
	runner.join ();
	{
		nano::wallets_error error = node1.wallets.attempt_password (wallet_id, "1234");
		if (error == nano::wallets_error::none)
		{
			nano::raw_key seed_now;
			(void)node1.wallets.get_seed (wallet_id, seed_now);
			ASSERT_EQ (seed_now, seed);
		}
		else
		{
			error = node1.wallets.attempt_password (wallet_id, "0000");
			if (error == nano::wallets_error::none)
			{
				nano::raw_key seed_now;
				(void)node1.wallets.get_seed (wallet_id, seed_now);
				ASSERT_EQ (seed_now, seed);
			}
			else
			{
				error = node1.wallets.attempt_password (wallet_id, "4567");
				if (error == nano::wallets_error::none)
				{
					nano::raw_key seed_now;
					(void)node1.wallets.get_seed (wallet_id, seed_now);
					ASSERT_EQ (seed_now, seed);
				}
				else
				{
					ASSERT_FALSE (true);
				}
			}
		}
	}
}

TEST (wallet, change_seed)
{
	nano::test::system system (1);
	auto & node1 (*system.nodes[0]);
	auto wallet_id = node1.wallets.first_wallet_id ();
	node1.wallets.enter_initial_password (wallet_id);
	nano::raw_key seed1;
	seed1 = 1;
	nano::public_key pub;
	uint32_t index (4);
	auto prv = nano::deterministic_key (seed1, index);
	pub = nano::pub_key (prv);
	(void)node1.wallets.insert_adhoc (wallet_id, nano::dev::genesis_key.prv, false);
	auto block (node1.wallets.send_action (wallet_id, nano::dev::genesis_key.pub, pub, 100));
	ASSERT_NE (nullptr, block);
	ASSERT_TIMELY (5s, nano::test::exists (*system.nodes[0], { block }));
	{
		nano::account first_account;
		uint32_t restored_count;
		ASSERT_EQ (nano::wallets_error::none, node1.wallets.change_seed (wallet_id, seed1, 0, first_account, restored_count));
		nano::raw_key seed2;
		(void)node1.wallets.get_seed (wallet_id, seed2);
		ASSERT_EQ (seed1, seed2);
	}
	ASSERT_TRUE (node1.wallets.exists (pub));
}

TEST (wallet, epoch_2_validation)
{
	nano::test::system system (1);
	auto & node (*system.nodes[0]);
	auto wallet_id = node.wallets.first_wallet_id ();

	// Upgrade the genesis account to epoch 2
	ASSERT_NE (nullptr, system.upgrade_genesis_epoch (node, nano::epoch::epoch_1));
	ASSERT_NE (nullptr, system.upgrade_genesis_epoch (node, nano::epoch::epoch_2));

	(void)node.wallets.insert_adhoc (wallet_id, nano::dev::genesis_key.prv, false);

	// Test send and receive blocks
	// An epoch 2 receive block should be generated with lower difficulty with high probability
	auto tries = 0;
	auto max_tries = 20;
	auto amount = node.config->receive_minimum.number ();
	while (++tries < max_tries)
	{
		auto send = node.wallets.send_action (wallet_id, nano::dev::genesis_key.pub, nano::dev::genesis_key.pub, amount, 1);
		ASSERT_NE (nullptr, send);
		ASSERT_EQ (nano::epoch::epoch_2, send->sideband ().details ().epoch ());
		ASSERT_EQ (nano::epoch::epoch_0, send->sideband ().source_epoch ()); // Not used for send state blocks

		auto receive = node.wallets.receive_action (wallet_id, send->hash (), nano::dev::genesis_key.pub, amount, send->destination (), 1);
		ASSERT_NE (nullptr, receive);
		if (nano::dev::network_params.work.difficulty (*receive) < node.network_params.work.get_base ())
		{
			ASSERT_GE (nano::dev::network_params.work.difficulty (*receive), node.network_params.work.get_epoch_2_receive ());
			ASSERT_EQ (nano::epoch::epoch_2, receive->sideband ().details ().epoch ());
			ASSERT_EQ (nano::epoch::epoch_2, receive->sideband ().source_epoch ());
			break;
		}
	}
	ASSERT_LT (tries, max_tries);

	// Test a change block
	ASSERT_NE (nullptr, node.wallets.change_action (wallet_id, nano::dev::genesis_key.pub, nano::keypair ().pub, 1));
}

// Receiving from an upgraded account uses the lower threshold and upgrades the receiving account
TEST (wallet, epoch_2_receive_propagation)
{
	auto tries = 0;
	auto const max_tries = 20;
	while (++tries < max_tries)
	{
		nano::test::system system;
		nano::node_flags node_flags;
		node_flags.set_disable_request_loop (true);
		auto & node (*system.add_node (node_flags));
		auto wallet_id = node.wallets.first_wallet_id ();

		// Upgrade the genesis account to epoch 1
		auto epoch1 = system.upgrade_genesis_epoch (node, nano::epoch::epoch_1);
		ASSERT_NE (nullptr, epoch1);

		nano::keypair key;
		nano::state_block_builder builder;

		// Send and open the account
		(void)node.wallets.insert_adhoc (wallet_id, nano::dev::genesis_key.prv, false);
		(void)node.wallets.insert_adhoc (wallet_id, key.prv, false);
		auto amount = node.config->receive_minimum.number ();
		auto send1 = node.wallets.send_action (wallet_id, nano::dev::genesis_key.pub, key.pub, amount, 1);
		ASSERT_NE (nullptr, send1);
		ASSERT_NE (nullptr, node.wallets.receive_action (wallet_id, send1->hash (), nano::dev::genesis_key.pub, amount, send1->destination (), 1));

		// Upgrade the genesis account to epoch 2
		auto epoch2 = system.upgrade_genesis_epoch (node, nano::epoch::epoch_2);
		ASSERT_NE (nullptr, epoch2);

		// Send a block
		auto send2 = node.wallets.send_action (wallet_id, nano::dev::genesis_key.pub, key.pub, amount, 1);
		ASSERT_NE (nullptr, send2);

		auto receive2 = node.wallets.receive_action (wallet_id, send2->hash (), key.pub, amount, send2->destination (), 1);
		ASSERT_NE (nullptr, receive2);
		if (nano::dev::network_params.work.difficulty (*receive2) < node.network_params.work.get_base ())
		{
			ASSERT_GE (nano::dev::network_params.work.difficulty (*receive2), node.network_params.work.get_epoch_2_receive ());
			ASSERT_EQ (nano::epoch::epoch_2, node.ledger.version (*node.store.tx_begin_read (), receive2->hash ()));
			ASSERT_EQ (nano::epoch::epoch_2, receive2->sideband ().source_epoch ());
			break;
		}
	}
	ASSERT_LT (tries, max_tries);
}

// Opening an upgraded account uses the lower threshold
TEST (wallet, epoch_2_receive_unopened)
{
	// Ensure the lower receive work is used when receiving
	auto tries = 0;
	auto const max_tries = 20;
	while (++tries < max_tries)
	{
		nano::test::system system;
		nano::node_flags node_flags;
		node_flags.set_disable_request_loop (true);
		auto & node (*system.add_node (node_flags));
		auto wallet_id = node.wallets.first_wallet_id ();

		// Upgrade the genesis account to epoch 1
		auto epoch1 = system.upgrade_genesis_epoch (node, nano::epoch::epoch_1);
		ASSERT_NE (nullptr, epoch1);

		nano::keypair key;
		nano::state_block_builder builder;

		// Send
		(void)node.wallets.insert_adhoc (wallet_id, nano::dev::genesis_key.prv, false);
		auto amount = node.config->receive_minimum.number ();
		auto send1 = node.wallets.send_action (wallet_id, nano::dev::genesis_key.pub, key.pub, amount, 1);

		// Upgrade unopened account to epoch_2
		auto epoch2_unopened = builder
							   .account (key.pub)
							   .previous (0)
							   .representative (0)
							   .balance (0)
							   .link (node.network_params.ledger.epochs.link (nano::epoch::epoch_2))
							   .sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
							   .work (*system.work.generate (key.pub, node.network_params.work.get_epoch_2 ()))
							   .build ();
		ASSERT_EQ (nano::block_status::progress, node.process (epoch2_unopened));

		(void)node.wallets.insert_adhoc (wallet_id, key.prv, false);

		auto receive1 = node.wallets.receive_action (wallet_id, send1->hash (), key.pub, amount, send1->destination (), 1);
		ASSERT_NE (nullptr, receive1);
		if (nano::dev::network_params.work.difficulty (*receive1) < node.network_params.work.get_base ())
		{
			ASSERT_GE (nano::dev::network_params.work.difficulty (*receive1), node.network_params.work.get_epoch_2_receive ());
			ASSERT_EQ (nano::epoch::epoch_2, node.ledger.version (*node.store.tx_begin_read (), receive1->hash ()));
			ASSERT_EQ (nano::epoch::epoch_1, receive1->sideband ().source_epoch ());
			break;
		}
	}
	ASSERT_LT (tries, max_tries);
}

/**
 * This test checks that wallets::foreach_representative can be used recursively
 */
TEST (wallet, foreach_representative_deadlock)
{
	nano::test::system system (1);
	auto & node (*system.nodes[0]);
	auto wallet_id = node.wallets.first_wallet_id ();
	(void)node.wallets.insert_adhoc (wallet_id, nano::dev::genesis_key.prv);
	node.wallets.compute_reps ();
	ASSERT_EQ (1, node.wallets.voting_reps_count ());

	bool set = false;
	node.wallets.foreach_representative ([&node, &set, &system] (nano::public_key const & pub, nano::raw_key const & prv) {
		node.wallets.foreach_representative ([&node, &set, &system] (nano::public_key const & pub, nano::raw_key const & prv) {
			ASSERT_TIMELY (5s, node.wallets.mutex.try_lock ().has_value ());
			set = true;
		});
	});
	ASSERT_TRUE (set);
}

TEST (wallet, search_receivable)
{
	nano::test::system system;
	nano::node_config config = system.default_config ();
	config.enable_voting = false;
	config.frontiers_confirmation = nano::frontiers_confirmation_mode::disabled;
	nano::node_flags flags;
	flags.set_disable_search_pending (true);
	auto & node (*system.add_node (config, flags));
	auto wallet_id = node.wallets.first_wallet_id ();

	(void)node.wallets.insert_adhoc (wallet_id, nano::dev::genesis_key.prv);
	nano::block_builder builder;
	auto send = builder.state ()
				.account (nano::dev::genesis_key.pub)
				.previous (nano::dev::genesis->hash ())
				.representative (nano::dev::genesis_key.pub)
				.balance (nano::dev::constants.genesis_amount - node.config->receive_minimum.number ())
				.link (nano::dev::genesis_key.pub)
				.sign (nano::dev::genesis_key.prv, nano::dev::genesis_key.pub)
				.work (*system.work.generate (nano::dev::genesis->hash ()))
				.build ();
	ASSERT_EQ (nano::block_status::progress, node.process (send));

	// Pending search should start an election
	ASSERT_TRUE (node.active.empty ());
	ASSERT_EQ (nano::wallets_error::none, node.wallets.search_receivable (wallet_id));
	std::shared_ptr<nano::election> election;
	ASSERT_TIMELY (5s, election = node.active.election (send->qualified_root ()));

	// Erase the key so the confirmation does not trigger an automatic receive
	auto genesis_account = nano::dev::genesis_key.pub;
	ASSERT_EQ (nano::wallets_error::none, node.wallets.remove_account (wallet_id, genesis_account));

	// Now confirm the election
	node.active.force_confirm (*election);

	ASSERT_TIMELY (5s, node.block_confirmed (send->hash ()) && node.active.empty ());

	// Re-insert the key
	(void)node.wallets.insert_adhoc (wallet_id, nano::dev::genesis_key.prv);

	// Pending search should create the receive block
	ASSERT_EQ (2, node.ledger.block_count ());
	ASSERT_EQ (nano::wallets_error::none, node.wallets.search_receivable (wallet_id));
	ASSERT_TIMELY_EQ (3s, node.balance (nano::dev::genesis_key.pub), nano::dev::constants.genesis_amount);
	auto receive_hash = node.ledger.any ().account_head (*node.store.tx_begin_read (), nano::dev::genesis_key.pub);
	auto receive = node.block (receive_hash);
	ASSERT_NE (nullptr, receive);
	ASSERT_EQ (receive->sideband ().height (), 3);
	ASSERT_EQ (send->hash (), receive->source ());
}

TEST (wallet, receive_pruned)
{
	nano::test::system system;
	nano::node_flags node_flags;
	node_flags.set_disable_request_loop (true);
	auto & node1 = *system.add_node (node_flags);
	node_flags.set_enable_pruning (true);
	nano::node_config config = system.default_config ();
	config.enable_voting = false; // Remove after allowing pruned voting
	auto & node2 = *system.add_node (config, node_flags);

	auto wallet_id1 = node1.wallets.first_wallet_id ();
	auto wallet_id2 = node2.wallets.first_wallet_id ();

	nano::keypair key;
	nano::state_block_builder builder;

	// Send
	(void)node1.wallets.insert_adhoc (wallet_id1, nano::dev::genesis_key.prv, false);
	auto amount = node2.config->receive_minimum.number ();
	auto send1 = node1.wallets.send_action (wallet_id1, nano::dev::genesis_key.pub, key.pub, amount, 1);
	auto send2 = node1.wallets.send_action (wallet_id1, nano::dev::genesis_key.pub, key.pub, 1, 1);

	// Pruning
	ASSERT_TIMELY_EQ (5s, node2.ledger.cemented_count (), 3);
	{
		auto transaction = node2.store.tx_begin_write ();
		ASSERT_EQ (1, node2.ledger.pruning_action (*transaction, send1->hash (), 2));
	}
	ASSERT_EQ (1, node2.ledger.pruned_count ());
	ASSERT_TRUE (node2.block_or_pruned_exists (send1->hash ()));
	ASSERT_FALSE (node2.ledger.any ().block_exists (*node2.store.tx_begin_read (), send1->hash ()));

	(void)node2.wallets.insert_adhoc (wallet_id2, key.prv, false);

	auto open1 = node2.wallets.receive_action (wallet_id2, send1->hash (), key.pub, amount, send1->destination (), 1);
	ASSERT_NE (nullptr, open1);
	ASSERT_EQ (amount, node2.ledger.any ().block_balance (*node2.store.tx_begin_read (), open1->hash ()));
	ASSERT_TIMELY_EQ (5s, node2.ledger.cemented_count (), 4);
}
