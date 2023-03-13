#pragma once

#include <mutex>

namespace nano
{
class random_pool
{
public:
	static void generate_block (unsigned char * output, size_t size);
	static unsigned generate_word32 (unsigned min, unsigned max);
	static unsigned char generate_byte ();

	/** Fills variable with random data */
	template <class T>
	static void generate (T & out)
	{
		generate_block (reinterpret_cast<uint8_t *> (&out), sizeof (T));
	}
	/** Returns variable with random data */
	template <class T>
	static T generate ()
	{
		T t;
		generate (t);
		return t;
	}

public:
	random_pool () = delete;
	random_pool (random_pool const &) = delete;
	random_pool & operator= (random_pool const &) = delete;

private:
	template <class Iter>
	friend void random_pool_shuffle (Iter begin, Iter end);
};
}
