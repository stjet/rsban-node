#pragma once

#include <nano/lib/numbers.hpp>
#include <nano/store/component.hpp>
#include <nano/store/iterator.hpp>

namespace nano
{
class block_hash;
}
namespace nano::store
{
/**
 * Manages peer storage and iteration
 */
class peer
{
public:
	virtual size_t count (store::transaction const & transaction_a) const = 0;
	virtual void clear (store::write_transaction const & transaction_a) = 0;
};
} // namespace nano::store
