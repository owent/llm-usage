# -*- coding: utf-8 -*-
"""Kimi Work collector: usage.record in the kimi-code engine session wire.jsonl.

Default root: D:/Cache/KimiDesktop/daimon-share/daimon/runtime/kimi-code/home/sessions.
KIMI_WORK_SESSIONS overrides it with semicolon-separated paths.
"""
import glob
import json
import os

from . import collector, iter_new_lines, rid, Event

TOOL = "kimi-work"


def _roots():
    env = os.environ.get("KIMI_WORK_SESSIONS", "")
    roots = [p for p in env.split(";") if p.strip()]
    if not roots:
        roots = [r"D:\Cache\KimiDesktop\daimon-share\daimon\runtime\kimi-code\home\sessions"]
    return roots


@collector(TOOL)
def collect(ctx):
    found = False
    events = []
    for root in _roots():
        if not os.path.isdir(root):
            continue
        for path in glob.glob(os.path.join(root, "wd_*", "*", "agents", "*", "wire.jsonl")):
            found = True
            for raw in iter_new_lines(path, ctx, TOOL):
                if '"usage.record"' not in raw:
                    continue
                try:
                    obj = json.loads(raw)
                except Exception:
                    continue
                if obj.get("type") != "usage.record":
                    continue
                u = obj.get("usage") or {}
                t = obj.get("time")
                if not t:
                    continue
                events.append(Event(
                    tool=TOOL,
                    request_id=rid(TOOL, path, raw),
                    ts=int(t) / 1000.0,
                    model=str(obj.get("model") or "unknown"),
                    provider="kimi",
                    input=int(u.get("inputOther") or 0),
                    output=int(u.get("output") or 0),
                    cache_read=int(u.get("inputCacheRead") or 0),
                    cache_write=int(u.get("inputCacheCreation") or 0),
                ))
    if not found:
        ctx.skip_reason = "未找到 kimi work 会话目录"
    return events
