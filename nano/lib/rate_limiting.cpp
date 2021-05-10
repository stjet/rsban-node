#include <nano/lib/rate_limiting.hpp>

extern "C" {
	void* rsn_token_bucket_create (size_t max_token_count, size_t refill_rate);
	void rsn_token_bucket_destroy (void* bucket);	
	void rsn_token_bucket_reset (void* bucket, size_t max_token_count, size_t refill_rate);	
	size_t rsn_token_bucket_largest_burst (void* bucket);
	bool rsn_token_bucket_try_consume (void* bucket, size_t tokens_required);
} 

nano::rate::token_bucket::token_bucket (size_t max_token_count_a, size_t refill_rate_a)
{
	bucket_handle = rsn_token_bucket_create (max_token_count_a, refill_rate_a);
}

nano::rate::token_bucket::~token_bucket (){
	rsn_token_bucket_destroy (bucket_handle);
}

bool nano::rate::token_bucket::try_consume (unsigned tokens_required_a)
{
	return rsn_token_bucket_try_consume (bucket_handle, static_cast<size_t>(tokens_required_a));
}

size_t nano::rate::token_bucket::largest_burst () const
{
	return rsn_token_bucket_largest_burst (bucket_handle);
}

void nano::rate::token_bucket::reset (size_t max_token_count_a, size_t refill_rate_a)
{
	rsn_token_bucket_reset (bucket_handle, max_token_count_a, refill_rate_a);
}
