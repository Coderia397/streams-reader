"""How many CPU threads this container may use.

`os.cpu_count()` reports the host (192 cores on the ZeroGPU nodes, notes/08), not the container's allowance, so torch
sized its pool for 192 threads on about 2 vCPU and oversubscribed. The allowance is the cgroup CPU quota when one is
set, otherwise the scheduler affinity mask.
"""
from __future__ import annotations

import contextlib
import math
import os
from collections.abc import Callable

CGROUP_V2_CPU_MAX = "/sys/fs/cgroup/cpu.max"
CGROUP_V1_QUOTA = "/sys/fs/cgroup/cpu/cpu.cfs_quota_us"
CGROUP_V1_PERIOD = "/sys/fs/cgroup/cpu/cpu.cfs_period_us"
THREADS_ENV = "NOLABS_THREADS"


def _read(path: str) -> str | None:
    try:
        with open(path, encoding="ascii") as f:
            return f.read().strip()
    except OSError:
        return None


def _ceil_ratio(quota: str, period: str) -> int | None:
    try:
        q, p = int(quota), int(period)
    except ValueError:
        return None
    if q <= 0 or p <= 0:
        return None
    return max(1, math.ceil(q / p))


def cgroup_cpus(read: Callable[[str], str | None] = _read) -> int | None:
    """CPUs granted by the cgroup quota, rounded up; None when unlimited or unknown."""
    v2 = read(CGROUP_V2_CPU_MAX)
    if v2:
        parts = v2.split()
        if len(parts) == 2 and parts[0] != "max":
            return _ceil_ratio(parts[0], parts[1])
        return None
    quota, period = read(CGROUP_V1_QUOTA), read(CGROUP_V1_PERIOD)
    if quota and period:
        return _ceil_ratio(quota, period)
    return None


def affinity_cpus() -> int | None:
    getter = getattr(os, "sched_getaffinity", None)
    if getter is None:
        return None
    try:
        return len(getter(0)) or None
    except OSError:
        return None


def cpu_allowance(read: Callable[[str], str | None] = _read, affinity: Callable[[], int | None] = affinity_cpus,
                  env: dict[str, str] | None = None) -> int:
    """Threads to use: `NOLABS_THREADS` when set, else the smaller of the cgroup quota and the affinity mask, else 1.
    Never `os.cpu_count()`."""
    env = os.environ if env is None else env
    override = env.get(THREADS_ENV, "").strip()
    if override.isdigit() and int(override) > 0:
        return int(override)
    known = [n for n in (cgroup_cpus(read), affinity()) if n]
    return min(known) if known else 1


def configure_threads(threads: int) -> None:
    """Size the native thread pools before torch starts them. Called at import time of app.py, before any model."""
    for var in ("OMP_NUM_THREADS", "MKL_NUM_THREADS", "OPENBLAS_NUM_THREADS"):
        os.environ[var] = str(threads)


def configure_torch(torch_module, threads: int) -> None:
    torch_module.set_num_threads(threads)
    # Raises once an earlier parallel call has started the pool; the intra-op setting above still applies.
    with contextlib.suppress(RuntimeError):
        torch_module.set_num_interop_threads(1)
