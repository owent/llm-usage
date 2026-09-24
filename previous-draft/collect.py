# -*- coding: utf-8 -*-
"""LLM 用量采集与统计入口。

两种运行方式：
1. Blueprint Automation 托管执行，调用 run(ctx)。
   ctx.input: {"refresh": bool, "tools": [名称...]}
2. 独立运行：python collect.py [--refresh] [--tool NAME ...] [--out PATH]
   增量采集后把统计快照写入 dashboard/data.json（供静态看板读取），
   并打印各数据源状态。数据库默认使用 data/usage.db。
"""
import os
import sys
import json
import traceback
import pkgutil
from datetime import datetime

BASE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, BASE)

import collectors  # noqa: E402
from collectors import REGISTRY, Ctx  # noqa: E402
from store import Store, hit_rate, iso_week, local_day  # noqa: E402

DB_PATH = os.path.join(BASE, "data", "usage.db")


def _load_collectors():
    """自动发现 collectors/ 包下的所有采集器模块——新增工具只需丢一个文件进来。"""
    for info in pkgutil.iter_modules(collectors.__path__):
        __import__(f"collectors.{info.name}")


def _row(period, tool, model, requests, inp, out, cr, cw):
    return {
        "period": period, "tool": tool, "model": model,
        "requests": int(requests), "input": int(inp), "output": int(out),
        "cacheRead": int(cr), "cacheWrite": int(cw),
        "hitRate": hit_rate(cr, inp),
    }


def _aggregate(rows, keyfn):
    agg = {}
    for r in rows:
        k = keyfn(r)
        a = agg.setdefault(k, [0, 0, 0, 0, 0])
        a[0] += r["requests"]; a[1] += r["input"]; a[2] += r["output"]
        a[3] += r["cacheRead"]; a[4] += r["cacheWrite"]
    return [_row(k[0], k[1], k[2], *a) for k, a in sorted(agg.items())]


def run(ctx):
    inp = {}
    if isinstance(ctx, dict):
        inp = ctx.get("input") or {}
    if isinstance(inp, str):
        try:
            inp = json.loads(inp) or {}
        except Exception:
            inp = {}
    if not isinstance(inp, dict):
        inp = {}
    refresh = bool(inp.get("refresh"))
    only = set(inp.get("tools") or [])

    _load_collectors()
    store = Store(DB_PATH)

    sources = []
    touched_days = []
    try:
        for name, fn in REGISTRY.items():
            if only and name not in only:
                continue
            c = Ctx(store=store, refresh=refresh)
            try:
                events = list(fn(c) or [])
                new_days = store.add_events(events)
                touched_days.extend(new_days)
                entry = {"tool": name, "status": "ok", "events": len(new_days)}
                if c.skip_reason:
                    entry["note"] = c.skip_reason
            except Exception as e:
                traceback.print_exc()
                entry = {"tool": name, "status": "error", "events": 0,
                         "error": f"{type(e).__name__}: {e}"}
            sources.append(entry)

        # 只重算有新事件的日期——历史数据直接沿用缓存
        store.rebuild_days(touched_days)
        store.prune()

        daily_rows = [_row(d, t, m, req, i, o, cr, cw)
                      for (d, t, m, req, i, o, cr, cw) in store._rows()]
        by_week = _aggregate(daily_rows, lambda r: (iso_week(r["period"]), r["tool"], r["model"]))
        by_month = _aggregate(daily_rows, lambda r: (r["period"][:7], r["tool"], r["model"]))
        today = local_day(datetime.now().timestamp())
        today_rows = [r for r in daily_rows if r["period"] == today]
        hourly_rows = [
            {"hour": f"{h:02d}", "tool": t, "model": m, "requests": int(req),
             "input": int(i), "output": int(o), "cacheRead": int(cr), "cacheWrite": int(cw)}
            for (h, t, m, req, i, o, cr, cw) in store.hourly_rows(today)
        ]

        return {"artifact": {
            "generatedAt": datetime.now().astimezone().isoformat(timespec="seconds"),
            "sources": sources,
            "byDay": daily_rows,
            "byWeek": by_week,
            "byMonth": by_month,
            "today": today_rows,
            "todayHourly": hourly_rows,
        }}
    finally:
        store.close()


def main(argv=None):
    import argparse
    parser = argparse.ArgumentParser(description="LLM 用量采集（独立模式）")
    parser.add_argument("--refresh", action="store_true",
                        help="忽略增量状态，全量重扫各数据源")
    parser.add_argument("--tool", dest="tools", action="append", default=None,
                        help="只运行指定采集器（可重复传入），如 --tool kimi-code")
    parser.add_argument("--out", default=os.path.join(BASE, "dashboard", "data.json"),
                        help="统计快照输出路径（默认 dashboard/data.json）")
    args = parser.parse_args(argv)

    inp = {"refresh": args.refresh}
    if args.tools:
        inp["tools"] = args.tools
    result = run({"input": inp})
    artifact = result["artifact"]

    out = os.path.abspath(args.out)
    os.makedirs(os.path.dirname(out), exist_ok=True)
    with open(out, "w", encoding="utf-8") as f:
        json.dump(artifact, f, ensure_ascii=False)

    print("生成时间:", artifact["generatedAt"])
    for s in artifact["sources"]:
        line = "  {tool}: {status}".format(**s)
        if s.get("events"):
            line += "（新增 {} 条）".format(s["events"])
        if s.get("note"):
            line += " - " + s["note"]
        if s.get("error"):
            line += " - " + s["error"]
        print(line)
    print("快照已写入:", out)
    return artifact


if __name__ == "__main__":
    main()
