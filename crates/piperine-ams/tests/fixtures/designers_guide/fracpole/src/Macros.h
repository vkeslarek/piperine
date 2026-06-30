#define BOOLEAN int
#define NO	0
#define YES	1

#define BIT0            0x0001
#define BIT1            0x0002

#include <assert.h>
#define ASSERT(cond)	assert(cond)
#define ALLOC(type, number)     \
    ((type *)malloc((unsigned) (sizeof(type)*(number))))
#define FREE(ptr)	(free((void *)(ptr)), (ptr) = 0)
#define ABORT()		{ abort(); /*NOTREACHED*/ }


