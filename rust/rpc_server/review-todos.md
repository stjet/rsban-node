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
 └── wallets
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
