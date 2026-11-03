#!/usr/bin/env python3
"""**A host with no free RAM is excluded, not ranked last** — and one that cannot prove its RAM is
excluded rather than assumed.

The picker's own docstring says this is check 3 of 5, and #195 measured why it matters: every loop
host on 2026-10-30 was out of *memory*, not CPU, and a build that takes six minutes took an hour.
An idle host with 0.4 GB available cannot link a Bevy test binary, so "most idle cores" alone picks
the machine that will fail.

What this pins:

1. **Below `--min-mem-gb` is excluded**, with the number in the reason.
2. **No `MemAvailable` series is an exclusion, not a zero** — "cannot prove it has RAM to link". A
   host that reports nothing is not a host with nothing free; the two mistakes cost different things,
   and the second silently drops a good machine.
3. **Among eligible hosts, memory breaks a tie** — two hosts with the same idle cores are ordered by
   available RAM, because the linker wants it and nothing else distinguishes them.

Run: `python3 -m unittest tools/loop/test_otel_host_pick.py`
"""

import importlib.util
import pathlib
import unittest

import argparse

_HERE = pathlib.Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location("otel_host_pick", _HERE / "otel_host_pick.py")
pick = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(pick)


def args(**kw):
    """The picker's own defaults, so a test that changes one says so."""
    base = dict(min_cores=4.0, free_cores=2.0, min_mem_gb=2.0, min_disk_gb=20.0, arch=None)
    base.update(kw)
    return argparse.Namespace(**base)


def host(name, **kw):
    """A host as `collect` builds one: every field the verdict and the sort read."""
    h = {
        "name": name,
        "instance": name,
        "ssh": name,
        "arch": "x86_64",
        "cores": 12.0,
        "cpu_pct": 10.0,
        "free_cores": 10.0,
        "load1": 1.0,
        "load5": 1.0,
        "load_per_core": 0.1,
        "mem_avail_gb": 16.0,
        "mem_total_gb": 32.0,
        "disk_avail_gb": 100.0,
        "up": True,
        "gpu": False,
    }
    h.update(kw)
    return h


class MemoryIsARequirement(unittest.TestCase):
    def test_below_the_floor_is_excluded_with_the_number(self):
        """#195's measurement: 0.5 GB free is a host that will page, not one that will build."""
        ok, why = pick.verdict(host("starved", mem_avail_gb=0.5), args())
        self.assertFalse(ok, "a host with 0.5 GB available must not be a candidate")
        self.assertIn("0.5", why, why)
        self.assertIn("--min-mem-gb", why, why)

    def test_no_series_is_an_exclusion_not_a_zero(self):
        """A host whose exporter publishes nothing about memory is unproven, not empty."""
        ok, why = pick.verdict(host("quiet", mem_avail_gb=None), args())
        self.assertFalse(ok, "a host with no MemAvailable series must not be a candidate")
        self.assertIn("no MemAvailable series", why, why)
        self.assertIn("cannot prove", why, why)

    def test_exactly_the_floor_is_eligible(self):
        """The floor is inclusive, so `--min-mem-gb 2` means "2 GB is enough"."""
        ok, why = pick.verdict(host("exact", mem_avail_gb=2.0), args())
        self.assertTrue(ok, why)

    def test_raising_the_floor_excludes_a_host_it_passed_before(self):
        """The flag is what a caller tunes for a bigger crate, so it must actually bite."""
        self.assertTrue(pick.verdict(host("sixteen", mem_avail_gb=16.0), args())[0])
        ok, why = pick.verdict(host("sixteen", mem_avail_gb=16.0), args(min_mem_gb=24.0))
        self.assertFalse(ok, "24 GB asked for, 16 GB available: not a candidate")
        self.assertIn("24", why, why)


class MemoryBreaksATie(unittest.TestCase):
    def test_equally_idle_hosts_are_ordered_by_available_ram(self):
        """Two hosts, the same idle cores: the linker picks the one with room, so the sort must."""
        eligible = [host("small", mem_avail_gb=3.0), host("roomy", mem_avail_gb=24.0)]
        eligible.sort(key=pick.ranking_key)
        self.assertEqual([h["name"] for h in eligible], ["roomy", "small"])

    def test_more_idle_cores_still_wins_over_more_ram(self):
        """The rule this script exists for: 12.9 free cores beat an idle-but-small machine."""
        eligible = [
            host("many", free_cores=12.9, load_per_core=0.1, mem_avail_gb=3.0),
            host("few", free_cores=0.6, load_per_core=9.4, mem_avail_gb=40.0),
        ]
        eligible.sort(key=pick.ranking_key)
        self.assertEqual(eligible[0]["name"], "many", "the 2026-10-26 measurement: cores first, then RAM")


if __name__ == "__main__":
    unittest.main()
