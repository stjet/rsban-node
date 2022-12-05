add_library(crypto_lib random_pool.hpp random_pool.cpp random_pool_shuffle.hpp
                       secure_memory.hpp secure_memory.cpp)

target_link_libraries(crypto_lib rsnano_ffi)

add_dependencies(crypto_lib cargo-build_rsnano_ffi)
