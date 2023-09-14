#pragma once

#include "nano/node/transport/channel.hpp"

#include <nano/node/common.hpp>

#include <boost/multi_index/hashed_index.hpp>
#include <boost/multi_index/member.hpp>
#include <boost/multi_index/ordered_index.hpp>
#include <boost/multi_index_container.hpp>

#include <deque>
#include <memory>

namespace mi = boost::multi_index;

namespace nano
{
class bootstrap_ascending_config;
class network_constants;
namespace bootstrap_ascending
{
	// Container for tracking and scoring peers with respect to bootstrapping
	class peer_scoring
	{
	public:
		peer_scoring (nano::bootstrap_ascending_config & config, nano::network_constants const & network_constants);
		// Returns true if channel limit has been exceeded
		bool try_send_message (std::shared_ptr<nano::transport::channel> channel);
		void received_message (std::shared_ptr<nano::transport::channel> channel);
		std::shared_ptr<nano::transport::channel> channel ();
		[[nodiscard]] std::size_t size () const;
		// Cleans up scores for closed channels
		// Decays scores which become inaccurate over time due to message drops
		void timeout ();
		void sync (std::deque<std::shared_ptr<nano::transport::channel>> const & list);

	private:
		class peer_score
		{
		public:
			explicit peer_score (std::shared_ptr<nano::transport::channel> const &, uint64_t, uint64_t, uint64_t);
			nano::transport::channel_weak_ptr channel;
			size_t channel_id;
			// Acquire reference to the shared channel object if it is still valid
			[[nodiscard]] std::shared_ptr<nano::transport::channel> shared () const;
			void decay ()
			{
				outstanding = outstanding > 0 ? outstanding - 1 : 0;
			}
			// Number of outstanding requests to a peer
			uint64_t outstanding{ 0 };
			uint64_t request_count_total{ 0 };
			uint64_t response_count_total{ 0 };
		};
		nano::network_constants const & network_constants;
		nano::bootstrap_ascending_config & config;

		// clang-format off
		// Indexes scores by their shared channel pointer
		class tag_channel {};
		// Indexes scores by the number of outstanding requests in ascending order
		class tag_outstanding {};

		using scoring_t = boost::multi_index_container<peer_score,
		mi::indexed_by<
			mi::hashed_unique<mi::tag<tag_channel>,
				mi::member<peer_score, size_t, &peer_score::channel_id>>,
			mi::ordered_non_unique<mi::tag<tag_outstanding>,
				mi::member<peer_score, uint64_t, &peer_score::outstanding>>>>;
		// clang-format on
		scoring_t scoring;
	};
}
}
