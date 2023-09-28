#pragma once

#include <nano/lib/rsnano.hpp>
#include <nano/store/db_val.hpp>

namespace nano::store::lmdb
{
using db_val = db_val<rsnano::MdbVal>;
}
