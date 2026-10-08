# -*- coding: utf-8 -*-
"""Zcode collector: ~/.zcode/cli/rollout/model-io-*.jsonl, one record per request
with camelCase response.usage fields."""
import glob
import json
import os
from datetime import datetime, timezone

from . import collector, iter_new_lines, Event

TOOL = "zcode"
HOME = os.path.expanduser("~")


def _ts(iso) -> float:
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
    for path in glob.glob(os.path.join(HOME, ".zcode", "cli", "rollout", "model-io-*.jsonl")):
        for raw in iter_new_lines(path, ctx, TOOL):
            if '"usage"' not in raw:
                continue
            try:
                obj = json.loads(raw)
            except Exception:
                continue
            u = (obj.get("response") or {}).get("usage")
            if not u:
                continue
            model = obj.get("model") or {}
            ts = _ts(obj.get("completedAt"))
            if not ts:
                continue
            events.append(Event(
                tool=TOOL,
                request_id=f"zcode:{obj.get('requestId') or obj.get('traceId')}",
                ts=ts,
                model=str(model.get("modelId") or "unknown"),
                provider=str(model.get("providerId") or ""),
                input=int(u.get("inputTokens") or 0),
                output=int(u.get("outputTokens") or 0),
                cache_read=int(u.get("cacheReadTokens") or 0),
                cache_write=int(u.get("cacheWriteTokens") or 0),
            ))
    return events
