#include <nano/lib/logging.hpp>
#include <nano/test_common/testutil.hpp>

#include <gtest/gtest.h>

#include <ostream>

using namespace std::chrono_literals;

namespace
{
struct non_copyable
{
	non_copyable () = default;
	non_copyable (non_copyable const &) = delete;
	non_copyable (non_copyable &&) = default;
	non_copyable & operator= (non_copyable const &) = delete;
	non_copyable & operator= (non_copyable &&) = default;

	friend std::ostream & operator<< (std::ostream & os, non_copyable const & nc)
	{
		os << "non_copyable";
		return os;
	}
};
}

namespace
{
struct non_moveable
{
	non_moveable () = default;
	non_moveable (non_moveable const &) = delete;
	non_moveable (non_moveable &&) = delete;
	non_moveable & operator= (non_moveable const &) = delete;
	non_moveable & operator= (non_moveable &&) = delete;

	friend std::ostream & operator<< (std::ostream & os, non_moveable const & nm)
	{
		os << "non_moveable";
		return os;
	}
};
}
