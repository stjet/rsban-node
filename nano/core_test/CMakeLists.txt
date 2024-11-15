add_executable(
  core_test
  entry.cpp
  difficulty.cpp
  ledger.cpp
  node.cpp
  unchecked_map.cpp
  vote_processor.cpp
  wallet.cpp
  wallets.cpp)

target_compile_definitions(
  core_test PRIVATE -DTAG_VERSION_STRING=${TAG_VERSION_STRING}
                    -DGIT_COMMIT_HASH=${GIT_COMMIT_HASH})

target_link_libraries(core_test test_common OpenSSL::SSL)

include_directories(${CMAKE_SOURCE_DIR}/submodules)
include_directories(${CMAKE_SOURCE_DIR}/submodules/gtest/googletest/include)
