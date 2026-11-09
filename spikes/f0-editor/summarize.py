#!/usr/bin/env python3
"""Turns a results JSON of the spike into Markdown tables.

    python3 -I summarize.py results/run-XXXX.json
"""
import json
import sys


def f(x):
    return "–" if x is None else f"{x:.2f}"


def main(path):
    r = json.load(open(path))
    print(f"- started: {r['started']}")
    print(f"- user agent: {r['userAgent']}")
    print(f"- keys per scenario: {r['keys']} (+30 warm-up, not counted)")
    print(f"- timer resolution: {r.get('timerResolutionMs')} ms, crossOriginIsolated: {r.get('crossOriginIsolated')}")
    print(f"- stalls (no frame within 1 s): {r.get('stalls')}")
    print(f"- cold start to measurement start: {r['coldStartMs']:.0f} ms")
    print()
    for action in ["type", "backspace", "enter+backspace"]:
        print(f"### {action}")
        print()
        print("| owner | view | where | note | n | proc median | proc p95 | proc p99 | proc max | owner p99 | DOM p99 | next paint median | next paint p99 | double rAF median | double rAF p99 | in first frame | reparsed |")
        print("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|")
        for x in r["rows"]:
            if x["action"] != action:
                continue
            print(
                f"| {x['variant']} | {x['view']} | {x['position']} | {x['note']} | {x['proc']['n']} "
                f"| {f(x['proc']['median'])} | {f(x['proc']['p95'])} | {f(x['proc']['p99'])} | {f(x['proc']['max'])} "
                f"| {f(x['owner']['p99'])} | {f(x['dom']['p99'])} "
                f"| {f(x['paint']['median'])} | {f(x['paint']['p99'])} "
                f"| {f(x['e2e']['median'])} | {f(x['e2e']['p99'])} "
                f"| {x['firstFrame'] * 100:.0f} % | {x['reparsed']} |"
            )
        print()
    print("### loads and switches")
    print()
    print("| owner | what | ms | detail |")
    print("|---|---|---|---|")
    groups = {}
    for l in r["loads"]:
        groups.setdefault((l["variant"], l["what"]), []).append(l)
    for (v, what), ls in groups.items():
        ms = sorted(l["ms"] for l in ls)
        val = f"{ms[len(ms) // 2]:.0f}" + (f" (median of {len(ms)}, max {ms[-1]:.0f})" if len(ms) > 1 else "")
        detail = ", ".join(
            f"{k} {v:.0f}" if isinstance(v, float) else f"{k} {v}" for k, v in (ls[0].get("detail") or {}).items()
        )
        print(f"| {v} | {what} | {val} | {detail} |")
    print()
    print("### checks")
    print()
    print("| owner | view | check | ok | detail |")
    print("|---|---|---|---|---|")
    for c in r["checks"]:
        print(f"| {c['variant']} | {c['view']} | {c['name']} | {'yes' if c['ok'] else '**no**'} | {c['detail']} |")


if __name__ == "__main__":
    main(sys.argv[1])
