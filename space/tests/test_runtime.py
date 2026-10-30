import os

import pytest

from nolabs_engine import runtime


def reader(files: dict[str, str]):
    return lambda path: files.get(path)


@pytest.mark.parametrize("files, expected", [
    ({runtime.CGROUP_V2_CPU_MAX: "200000 100000"}, 2),
    ({runtime.CGROUP_V2_CPU_MAX: "150000 100000"}, 2),
    ({runtime.CGROUP_V2_CPU_MAX: "50000 100000"}, 1),
    ({runtime.CGROUP_V2_CPU_MAX: "max 100000"}, None),
    ({runtime.CGROUP_V2_CPU_MAX: "garbage"}, None),
    ({runtime.CGROUP_V1_QUOTA: "400000", runtime.CGROUP_V1_PERIOD: "100000"}, 4),
    ({runtime.CGROUP_V1_QUOTA: "-1", runtime.CGROUP_V1_PERIOD: "100000"}, None),
    ({}, None),
])
def test_cgroup_quota(files, expected):
    assert runtime.cgroup_cpus(reader(files)) == expected


def test_allowance_takes_the_smaller_limit_and_never_cpu_count(monkeypatch):
    monkeypatch.setattr(os, "cpu_count", lambda: 192)
    two = reader({runtime.CGROUP_V2_CPU_MAX: "200000 100000"})
    assert runtime.cpu_allowance(two, lambda: 192, env={}) == 2
    assert runtime.cpu_allowance(reader({}), lambda: 3, env={}) == 3
    assert runtime.cpu_allowance(reader({}), lambda: None, env={}) == 1
    assert runtime.cpu_allowance(two, lambda: 192, env={"NOLABS_THREADS": "6"}) == 6
    assert runtime.cpu_allowance(two, lambda: 192, env={"NOLABS_THREADS": "zero"}) == 2


def test_configure_threads_sets_pool_env(monkeypatch):
    for var in ("OMP_NUM_THREADS", "MKL_NUM_THREADS", "OPENBLAS_NUM_THREADS"):
        monkeypatch.delenv(var, raising=False)
    runtime.configure_threads(2)
    assert os.environ["OMP_NUM_THREADS"] == "2" and os.environ["MKL_NUM_THREADS"] == "2"


def test_configure_torch_tolerates_late_interop_setting():
    calls = []

    class FakeTorch:
        def set_num_threads(self, n):
            calls.append(("intra", n))

        def set_num_interop_threads(self, n):
            raise RuntimeError("cannot set after parallel work started")

    runtime.configure_torch(FakeTorch(), 2)
    assert calls == [("intra", 2)]
