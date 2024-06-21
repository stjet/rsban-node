add_library(
  secure
  ${PLATFORM_SECURE_SOURCE}
  account_info.hpp
  account_info.cpp
  common.hpp
  common.cpp
  generate_cache_flags.hpp
  generate_cache_flags.cpp
  ledger.hpp
  ledger.cpp
  network_filter.hpp
  network_filter.cpp
  pending_info.hpp
  pending_info.cpp
  utility.hpp
  utility.cpp)

target_link_libraries(secure nano_lib crypto_lib Boost::system)

target_compile_definitions(secure PUBLIC -DQT_NO_KEYWORDS
                                         -DBOOST_ASIO_HAS_STD_ARRAY=1)

include_directories(${CMAKE_SOURCE_DIR}/submodules)
