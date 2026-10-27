"""bd-cache in process: the Rust cache of a beads repository.

The native module is ``bd_cache_rs._native`` (maturin's mixed layout,
ADR-PY-0006); this package re-exports every name it defines, with the same
``__all__``, ``__version__`` and ``CONTRACT_VERSION``, so ``import
bd_cache_rs`` and ``bd_cache_rs.Cache`` work unchanged. Two entry points live
beside it:

- ``python -m bd_cache_rs.run post_sync <module:function>``: the post-sync
  hook's runner (``bd_cache_rs.run``);
- ``bd_cache_rs.sync.main(func)``: the harness a consumer's sync script calls.
"""

from . import _native
from ._native import *  # noqa: F401,F403 - every native name, re-exported

# The native module's public names (its private run marker left out).
__all__ = [
    n
    for n in getattr(_native, "__all__", dir(_native))
    if not n.startswith("_") or n == "__version__"
]
__version__ = _native.__version__
CONTRACT_VERSION = _native.CONTRACT_VERSION
