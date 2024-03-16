#pragma once

#include <nano/lib/block_type.hpp>

#include <boost/system/error_code.hpp>

#include <memory>
#include <vector>

namespace rsnano
{
class async_runtime;
}
namespace nano
{
class block;
namespace transport
{
	class socket;
}

namespace bootstrap
{
	/**
	 * Class to read a block-type byte followed by a serialised block from a stream.
	 * It is typically used to used to read a series of block-types and blocks terminated by a not-a-block type.
	 */
	class block_deserializer : public std::enable_shared_from_this<nano::bootstrap::block_deserializer>
	{
	public:
		using callback_type = std::function<void (boost::system::error_code, std::shared_ptr<nano::block>)>;

		block_deserializer (rsnano::async_runtime const & async_rt);
		block_deserializer (block_deserializer const &) = delete;
		~block_deserializer ();
		/**
		 * Read a type-prefixed block from 'socket' and pass the result, or an error, to 'callback'
		 * A normal end to series of blocks is a marked by return no error and a nullptr for block.
		 */
		void read (nano::transport::socket & socket, callback_type const && callback);

		rsnano::BlockDeserializerHandle * handle;
	};
}
}
