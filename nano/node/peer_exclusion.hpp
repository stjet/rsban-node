#include <nano/boost/asio/ip/tcp.hpp>

namespace rsnano
{
class PeerExclusionHandle;
}

namespace nano
{
class container_info_component;
using tcp_endpoint = boost::asio::ip::tcp::endpoint;

class peer_exclusion final
{
public:
	peer_exclusion (std::size_t max_size = 5000);
	peer_exclusion (nano::peer_exclusion const &) = delete;
	~peer_exclusion ();
	uint64_t add (nano::tcp_endpoint const &);
	uint64_t score (nano::tcp_endpoint const &) const;
	bool check (nano::tcp_endpoint const &) const;
	void remove (nano::tcp_endpoint const &);
	bool contains (nano::tcp_endpoint const &);
	std::size_t size () const;
	std::unique_ptr<container_info_component> collect_container_info (std::string const & name);

private:
	rsnano::PeerExclusionHandle * handle;
};
}
