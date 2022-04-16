import functools


@functools.lru_cache
def random(seed):
    # NOTE: as long as we use a deterministic seed, the sequence of random
    # numbers will be deterministic. This is important for hermeticity.

    # NOTE: if we ever want to parallelize the generation, it will be important
    # to consider how the random number sequence is affected.
    import random

    return random.Random(seed)
