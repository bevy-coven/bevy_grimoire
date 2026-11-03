#!/usr/bin/env python3
"""Ask Home Cloud's OTEL, not a guess, which device can take a compile right now.

**Why this exists.** Work got put on a saturated machine because the choice was a habit: the
laptop that happens to hold the checkout. Measured on 2026-10-26 with this script's own numbers,
`Cs-MacBook-Pro` was at 94% CPU with 0.6 cores free (two loops and a cargo build on ten cores)
while `bmb` sat at 7.9% with 12.9 free. A build sent to the first finished hours later.

**What "capable" means here, in the order the checks are applied** — a device is a candidate only
if all of them hold, because a machine that cannot build at all is worse than a busy one:

1. reachable (`up == 1`),
2. at least `--min-cores` cores, and at least `--free-cores` cores idle in a 5-minute average,
3. at least `--min-mem-gb` of `MemAvailable` (cargo linkers are the memory hog; a host with no free
   RAM reports 0 here and is excluded rather than ranked last),
4. `--arch` matches the target when given (`arm64`/`aarch64` are treated as one),
5. enough disk *if the series exists*: `--min-disk-gb` against `node_filesystem_avail_bytes`.
   A Bevy `target/` runs tens of GB. When a host reports no filesystem series this check is
   skipped and the reason says so, rather than silently passing.

**Load is a ratio, not a raw count.** `free_cores = cores × (1 − busy)` from
`rate(node_cpu_seconds_total{mode="idle"}[5m])`, and `load_per_core = load1 / cores`. Ranking by
raw `load1` would always pick the 24-core host over an idle 10-core one, which is the mistake this
script exists to stop.

Usage:
  otel_host_pick.py                       # rank every host
  otel_host_pick.py --arch arm64          # only Apple Silicon
  otel_host_pick.py --need-cores 6        # for `cargo build -p <crate>`, not a full workspace
  otel_host_pick.py --json                # machine-readable, for another script
  otel_host_pick.py --pick                # print one ssh destination, nothing else

Exit codes: 0 a host was found, 1 no host met the requirements, 2 OTEL unreachable.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import urllib.parse
import urllib.request

DEFAULT_PROM = "http://192.168.1.104:9090"


def query(prom: str, expr: str) -> list[dict]:
    url = f"{prom}/api/v1/query?" + urllib.parse.urlencode({"query": expr})
    with urllib.request.urlopen(url, timeout=10) as r:
        body = json.load(r)
    if body.get("status") != "success":
        raise RuntimeError(f"prometheus refused {expr!r}: {body}")
    return body["data"]["result"]


def series(prom: str, expr: str) -> dict[str, float]:
    """One scalar per instance. A series that is absent stays absent, so a missing metric can never
    read as a perfect score — the difference between 'no data' and 'idle' matters."""
    out: dict[str, float] = {}
    for r in query(prom, expr):
        inst = r["metric"].get("instance")
        if inst is None or len(r["value"]) < 2:
            continue
        try:
            out[inst] = float(r["value"][1])
        except ValueError:
            continue
    return out


def norm_arch(a: str) -> str:
    a = (a or "").lower()
    return "arm64" if a in ("arm64", "aarch64") else a


def collect(prom: str) -> list[dict]:
    up = series(prom, "up")
    info = {}
    for r in query(prom, "node_uname_info"):
        inst = r["metric"].get("instance")
        if inst:
            info[inst] = r["metric"]
    cores = series(prom, 'count by (instance)(node_cpu_seconds_total{mode="idle"})')
    busy = series(prom, '100 * (1 - avg by (instance)(rate(node_cpu_seconds_total{mode="idle"}[5m])))')
    load1 = series(prom, "node_load1")
    load5 = series(prom, "node_load5")
    # **Memory comes from two different exporters, and the case of the metric name is the tell.**
    # Linux's node_exporter publishes `node_memory_MemAvailable_bytes` / `node_memory_MemTotal_bytes`;
    # the macOS exporter publishes lowercase `node_memory_free_bytes` / `node_memory_inactive_bytes`
    # / `node_memory_total_bytes` and no `MemAvailable` at all. Measured on 2026-10-26: reading only
    # the Linux names excluded every Mac from ranking — including `bmb`, which had 12.9 free cores
    # and was the right answer. A missing series is `None` (unknown), never 0, so it can be reported
    # as unknown instead of as "no memory".
    mem_avail = series(prom, "node_memory_MemAvailable_bytes")
    mem_total = series(prom, "node_memory_MemTotal_bytes")
    mac_free = series(prom, "node_memory_free_bytes")
    mac_inactive = series(prom, "node_memory_inactive_bytes")
    mac_total = series(prom, "node_memory_total_bytes")
    mac_nominal = series(prom, "node_memory_total_macos_bytes")  # older exporter spelling
    for inst in set(mac_free) | set(mac_total) | set(mac_nominal):
        if inst not in mem_avail:
            # `free + inactive` is the closest equivalent of "available": inactive pages are
            # reclaimable, which is exactly what a build needs. `free` alone reads as ~0.3 GB on a
            # healthy 48 GB Mac and would refuse every host.
            mem = mac_free.get(inst, 0.0) + mac_inactive.get(inst, 0.0)
            if mem:
                mem_avail[inst] = mem
        if inst not in mem_total:
            tot = mac_total.get(inst) or mac_nominal.get(inst)
            if tot:
                mem_total[inst] = tot
    # **Pick the mount cargo would actually write to, not the biggest one.** `max()` over all
    # mounts is wrong twice over, both measured on 2026-10-26: `bmb` reported 191.5 GB from a
    # `/private/var/folders/.../AppTranslocation/...` nullfs stub while its `/` had 22.3 GB, and a
    # host reported 1048576.0 GB (a PB) from the same kind of junk mount. `/System/Volumes/Data`
    # first because on APFS `/` is a read-only system volume and the data volume is where `$HOME`
    # and `target/` live; then `/`; then the largest of what remains.
    PREFERRED = ["/System/Volumes/Data", "/", "/home", "/var"]
    disk_raw: dict[str, dict[str, float]] = {}
    for r in query(prom, 'node_filesystem_avail_bytes{fstype!~"tmpfs|devtmpfs|overlay|squashfs|nullfs|autofs|proc|sysfs"}'):
        inst = r["metric"].get("instance")
        mp = r["metric"].get("mountpoint", "")
        if not inst or len(r["value"]) < 2:
            continue
        try:
            size = float(r["value"][1])
        except ValueError:
            continue
        if size <= 0:
            continue
        disk_raw.setdefault(inst, {})[mp] = size
    disk: dict[str, float] = {}
    for inst, mounts in disk_raw.items():
        for mp in PREFERRED:
            if mp in mounts:
                disk[inst] = mounts[mp]
                break
        else:
            disk[inst] = max(mounts.values())
    gpu = series(prom, 'max by (instance)(nvidia_gpu_exporter_build_info)')

    hosts = []
    for inst in sorted(cores):
        m = info.get(inst, {})
        c = cores.get(inst, 0.0)
        b = busy.get(inst, 0.0)
        l1 = load1.get(inst, 0.0)
        avail = mem_avail.get(inst)
        total = mem_total.get(inst)
        hosts.append({
            "name": m.get("nodename") or inst,
            "instance": inst,
            "ssh": (m.get("nodename") or inst).replace(".local", ""),
            "arch": norm_arch(m.get("machine", "")),
            "cores": c,
            "cpu_pct": round(b, 1),
            "free_cores": round(c * (1 - b / 100.0), 1),
            "load1": round(l1, 2),
            "load5": round(load5.get(inst, 0.0), 2),
            "load_per_core": round(l1 / c, 2) if c else None,
            "mem_avail_gb": round(avail / 2**30, 1) if avail is not None else None,
            "mem_total_gb": round(total / 2**30, 1) if total is not None else None,
            "disk_avail_gb": round(disk[inst] / 2**30, 1) if inst in disk else None,
            "up": up.get(inst, 1.0) == 1.0,
            "gpu": inst in gpu,
        })
    return hosts


def ranking_key(h: dict) -> tuple:
    """**The order candidates are tried in**, one expression so a test can hold it.

    Most idle cores first, then least loaded per core, then most RAM. **Cores lead** because that is
    the mistake this script exists to stop — the 2026-10-26 measurement had a 10-core laptop at 0.6
    free cores beating a 14-core Mac at 12.9 — and **RAM breaks the tie**, because two equally idle
    hosts are not equally able to link a Bevy test binary (#195: every loop host was short of
    memory, not CPU).
    """
    return (-h["free_cores"], h["load_per_core"] or 9e9, -(h["mem_avail_gb"] or 0))


def verdict(h: dict, args) -> tuple[bool, str]:
    """The first failure names itself; a silent exclusion is how a good host gets skipped."""
    if not h["up"]:
        return False, "down (exporters report up=0)"
    if h["cores"] < args.min_cores:
        return False, f"{int(h['cores'])} cores < --min-cores {args.min_cores}"
    if h["free_cores"] < args.free_cores:
        return False, f"{h['free_cores']} free cores < --free-cores {args.free_cores}"
    if h["mem_avail_gb"] is None:
        return False, "no MemAvailable series — cannot prove it has RAM to link"
    if h["mem_avail_gb"] < args.min_mem_gb:
        return False, f"{h['mem_avail_gb']} GB available < --min-mem-gb {args.min_mem_gb}"
    if args.arch and h["arch"] != norm_arch(args.arch):
        return False, f"arch {h['arch']} != --arch {norm_arch(args.arch)}"
    if args.min_disk_gb and h["disk_avail_gb"] is not None and h["disk_avail_gb"] < args.min_disk_gb:
        return False, f"{h['disk_avail_gb']} GB disk < --min-disk-gb {args.min_disk_gb}"
    if args.min_disk_gb and h["disk_avail_gb"] is None:
        return True, "disk unmeasured (no filesystem series); other checks passed"
    return True, "ok"


def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--prometheus", default=DEFAULT_PROM, help=f"Prometheus base URL (default {DEFAULT_PROM})")
    p.add_argument("--min-cores", "--need-cores", dest="min_cores", type=float, default=4.0)
    p.add_argument("--free-cores", type=float, default=2.0, help="cores idle in a 5m average")
    p.add_argument("--min-mem-gb", type=float, default=2.0, help="MemAvailable")
    p.add_argument("--min-disk-gb", type=float, default=20.0, help="free space where target/ goes")
    p.add_argument("--arch", default=None, help="arm64/aarch64/x86_64")
    p.add_argument("--json", action="store_true")
    p.add_argument("--pick", action="store_true", help="print the best ssh destination only")
    p.add_argument("--all", action="store_true", help="list excluded hosts and why")
    args = p.parse_args(argv)

    try:
        hosts = collect(args.prometheus)
    except Exception as e:  # unreachable OTEL must be loud: picking blind is the failure
        print(f"OTEL unreachable at {args.prometheus}: {e}", file=sys.stderr)
        return 2
    if not hosts:
        print(f"OTEL at {args.prometheus} reports no node hosts", file=sys.stderr)
        return 2

    scored = []
    for h in hosts:
        ok, why = verdict(h, args)
        h["eligible"], h["reason"] = ok, why
        scored.append(h)
    eligible = [h for h in scored if h["eligible"]]
    # Most idle cores first, then least loaded per core, then most RAM.
    eligible.sort(key=ranking_key)

    if args.json:
        print(json.dumps({"eligible": eligible, "all": scored}, indent=2))
        return 0 if eligible else 1
    if args.pick:
        if not eligible:
            print("no host meets the requirements", file=sys.stderr)
            return 1
        print(eligible[0]["ssh"])
        return 0

    print(f"{'host':<18}{'arch':<8}{'cores':>6}{'cpu%':>7}{'load/c':>8}{'freeC':>7}{'memAvail':>10}{'disk':>8}  verdict")
    for h in sorted(scored, key=lambda h: (not h["eligible"], -(h["free_cores"]))):
        mem = "?" if h["mem_avail_gb"] is None else f"{h['mem_avail_gb']}G"
        disk = "-" if h["disk_avail_gb"] is None else f"{h['disk_avail_gb']}G"
        lpc = "-" if h["load_per_core"] is None else f"{h['load_per_core']}"
        mark = "PICK" if eligible and h is eligible[0] else ("ok" if h["eligible"] else "no")
        print(f"{h['name']:<18}{h['arch']:<8}{int(h['cores']):>6}{h['cpu_pct']:>7}{lpc:>8}"
              f"{h['free_cores']:>7}{mem:>10}{disk:>8}  {mark}: {h['reason']}")
    if not eligible:
        print("\nno host meets the requirements — lower --free-cores or --min-mem-gb, or wait", file=sys.stderr)
        return 1
    print(f"\nbest: ssh {eligible[0]['ssh']}   ({eligible[0]['free_cores']} free cores, "
          f"{eligible[0]['load_per_core']} load/core, {eligible[0]['mem_avail_gb']}G available)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
