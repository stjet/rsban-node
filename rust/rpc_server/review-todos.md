# RPC functions MISSING:
- work_peer_add
- work_peers
- work_peers_clear
- bootstrap_status
- database_txn_tracker
- election_statistics
- pruned_exists
- receive_minimum_set

# Fix DTOS:
src
 ├── ledger
 │   ├── account_block_count.rs
 │   ├── account_history.rs
 │   ├── account_info.rs
 │   ├── account_representative.rs
 │   ├── account_weight.rs
 │   ├── accounts_balances.rs
 │   ├── accounts_frontiers.rs
 │   ├── accounts_receivable.rs
 │   ├── accounts_representatives.rs
 │   ├── available_supply.rs
 │   ├── block_account.rs
 │   ├── block_confirm.rs
 │   ├── block_count.rs
 │   ├── block_info.rs
 │   ├── blocks.rs
 │   ├── blocks_info.rs
 │   ├── chain.rs
 │   ├── delegators.rs
 │   ├── delegators_count.rs
 │   ├── frontier_count.rs
 │   ├── frontiers.rs
 │   ├── ledger.rs
 │   ├── mod.rs
 │   ├── representatives.rs
 │   ├── successors.rs
 │   └── unopened.rs
 ├── lib.rs
 ├── node
 │   ├── active_difficulty.rs
 │   ├── block_create.rs
 │   ├── bootstrap.rs
 │   ├── bootstrap_any.rs
 │   ├── bootstrap_lazy.rs
 │   ├── confirmation_active.rs
 │   ├── confirmation_history.rs
 │   ├── confirmation_info.rs
 │   ├── confirmation_quorum.rs
 │   ├── keepalive.rs
 │   ├── mod.rs
 │   ├── node_id.rs
 │   ├── peers.rs
 │   ├── populate_backlog.rs
 │   ├── process.rs
 │   ├── receivable.rs
 │   ├── receivable_exists.rs
 │   ├── representatives_online.rs
 │   ├── republish.rs
 │   ├── sign.rs
 │   ├── stats.rs
 │   ├── stats_clear.rs
 │   ├── stop.rs
 │   ├── telemetry.rs
 │   ├── unchecked.rs
 │   ├── unchecked_clear.rs
 │   ├── unchecked_get.rs
 │   ├── unchecked_keys.rs
 │   ├── uptime.rs
 │   ├── version.rs
 │   ├── work_cancel.rs
 │   ├── work_generate.rs
 │   ├── work_peer_add.rs
 │   ├── work_peers.rs
 │   ├── work_peers_clear.rs
 │   └── work_validate.rs
 ├── utils
 │   ├── account_get.rs
 │   ├── account_key.rs
 │   ├── block_hash.rs
 │   ├── deterministic_key.rs
 │   ├── key_create.rs
 │   ├── key_expand.rs
 │   ├── mod.rs
 │   ├── nano_to_raw.rs
 │   ├── raw_to_nano.rs
 │   └── validate_account_number.rs
 └── wallets
     ├── account_create.rs
     ├── account_list.rs
     ├── account_move.rs
     ├── account_remove.rs
     ├── accounts_create.rs
     ├── mod.rs
     ├── password_change.rs
     ├── password_enter.rs
     ├── password_valid.rs
     ├── receive.rs
     ├── receive_minimum.rs
     ├── search_receivable.rs
     ├── search_receivable_all.rs
     ├── send.rs
     ├── wallet_add.rs
     ├── wallet_add_watch.rs
     ├── wallet_balances.rs
     ├── wallet_change_seed.rs
     ├── wallet_contains.rs
     ├── wallet_create.rs
     ├── wallet_destroy.rs
     ├── wallet_export.rs
     ├── wallet_frontiers.rs
     ├── wallet_history.rs
     ├── wallet_info.rs
     ├── wallet_ledger.rs
     ├── wallet_lock.rs
     ├── wallet_locked.rs
     ├── wallet_receivable.rs
     ├── wallet_representative.rs
     ├── wallet_representative_set.rs
     ├── wallet_republish.rs
     ├── wallet_work_get.rs
     ├── work_get.rs
     └── work_set.rs
