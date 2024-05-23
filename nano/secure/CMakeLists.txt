if(${CMAKE_SYSTEM_NAME} MATCHES "Darwin")
  set(PLATFORM_SECURE_SOURCE plat/osx/working.mm)
elseif(${CMAKE_SYSTEM_NAME} MATCHES "Windows")
  set(PLATFORM_SECURE_SOURCE plat/windows/working.cpp)
elseif(${CMAKE_SYSTEM_NAME} MATCHES "Linux")
  set(PLATFORM_SECURE_SOURCE plat/posix/working.cpp)
elseif(${CMAKE_SYSTEM_NAME} MATCHES "FreeBSD")
  set(PLATFORM_SECURE_SOURCE plat/posix/working.cpp)
else()
  error("Unknown platform: ${CMAKE_SYSTEM_NAME}")
endif()

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
  ledger_cache.hpp
  ledger_cache.cpp
  network_filter.hpp
  network_filter.cpp
  pending_info.hpp
  pending_info.cpp
  rep_weights.hpp
  rep_weights.cpp
  utility.hpp
  utility.cpp
  working.hpp)

target_link_libraries(secure nano_lib crypto_lib Boost::system)

target_compile_definitions(secure PUBLIC -DQT_NO_KEYWORDS
                                         -DBOOST_ASIO_HAS_STD_ARRAY=1)

include_directories(${CMAKE_SOURCE_DIR}/submodules)
