add_executable(
  core_test
  entry.cpp
  fakes/websocket_client.hpp
  active_elections.cpp
  backlog.cpp
  block.cpp
  block_store.cpp
  bootstrap.cpp
  bootstrap_ascending.cpp
  cli.cpp
  confirming_set.cpp
  conflicts.cpp
  difficulty.cpp
  distributed_work.cpp
  election.cpp
  election_scheduler.cpp
  frontiers_confirmation.cpp
  ledger.cpp
  ledger_confirm.cpp
  network.cpp
  node.cpp
  object_stream.cpp
  optimistic_scheduler.cpp
  system.cpp
  telemetry.cpp
  uint256_union.cpp
  unchecked_map.cpp
  vote_processor.cpp
  voting.cpp
  wallet.cpp
  wallets.cpp
  websocket.cpp)

target_compile_definitions(
  core_test PRIVATE -DTAG_VERSION_STRING=${TAG_VERSION_STRING}
                    -DGIT_COMMIT_HASH=${GIT_COMMIT_HASH})

target_link_libraries(core_test test_common OpenSSL::SSL)

include_directories(${CMAKE_SOURCE_DIR}/submodules)
include_directories(${CMAKE_SOURCE_DIR}/submodules/gtest/googletest/include)
