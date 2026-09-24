# -*- coding: utf-8 -*-
"""oh-my-pi 采集器：~/.omp/agent/sessions/*/*.jsonl 主请求 + ~/.omp/logs/*.log 标题生成等辅助请求。"""
import glob
import json
import os
from datetime import datetime, timezone

from . import collector, iter_new_lines, rid, Event

TOOL = "oh-my-pi"
HOME = os.path.expanduser("~")


def _ts(iso: str) -> float:
    try:
        dt = datetime.fromisoformat(str(iso).replace("Z", "+00:00"))
        if dt.tzinfo is None:
            dt = dt.replace(tzinfo=timezone.utc)
        return dt.timestamp()
    except Exception:
        return 0.0


@collector(TOOL)
def collect(ctx):
    events = []
    # 1) 会话主请求
    for path in glob.glob(os.path.join(HOME, ".omp", "agent", "sessions", "*", "*.jsonl")):
        for raw in iter_new_lines(path, ctx, TOOL):
            if '"usage"' not in raw:
                continue
            try:
                obj = json.loads(raw)
            except Exception:
                continue
            if obj.get("type") != "message":
                continue
            msg = obj.get("message") or {}
            if msg.get("role") != "assistant" or not msg.get("usage"):
                continue
            ts = _ts(obj.get("timestamp") or msg.get("timestamp"))
            if not ts:
                continue
            u = msg["usage"]
            events.append(Event(
                tool=TOOL,
                request_id=rid(TOOL, path, obj.get("id")),
                ts=ts,
                model=str(msg.get("model") or "unknown"),
                provider=str(msg.get("provider") or ""),
                input=int(u.get("input") or 0),
                output=int(u.get("output") or 0),
                cache_read=int(u.get("cacheRead") or 0),
                cache_write=int(u.get("cacheWrite") or 0),
            ))
    # 2) 日志中的辅助请求（title-generator 等）
    for path in glob.glob(os.path.join(HOME, ".omp", "logs", "omp.*.log")):
        for raw in iter_new_lines(path, ctx, TOOL):
            if "title-generator" not in raw or '"usage"' not in raw:
                continue
            try:
                obj = json.loads(raw)
            except Exception:
                continue
            if obj.get("message") != "title-generator: success":
                continue
            u = obj.get("usage") or {}
            ts = _ts(obj.get("timestamp"))
            if not ts:
                continue
            model_full = str(obj.get("model") or "unknown")
            events.append(Event(
                tool=TOOL,
                request_id=rid(TOOL, path, obj.get("sessionId"), obj.get("timestamp"), u.get("output")),
                ts=ts,
                model=model_full.split("/")[-1],
                provider="",
                input=int(u.get("input") or 0),
                output=int(u.get("output") or 0),
                cache_read=int(u.get("cacheRead") or 0),
                cache_write=int(u.get("cacheWrite") or 0),
            ))
    return events
