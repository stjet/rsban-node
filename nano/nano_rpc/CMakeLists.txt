add_executable(nano_rpc entry.cpp)

target_link_libraries(nano_rpc node)

target_compile_definitions(
  nano_rpc
  PUBLIC -DACTIVE_NETWORK=${ACTIVE_NETWORK}
  PRIVATE -DTAG_VERSION_STRING=${TAG_VERSION_STRING}
          -DGIT_COMMIT_HASH=${GIT_COMMIT_HASH})
