# -*- coding: utf-8 -*-
"""New Kimi Code collector (v2.x, desktop/standalone CLI).

The new Kimi Code home moved from the shared kimi-code engine directory to
~/.kimi-code (for example C:/Users/<user>/.kimi-code). Session events still live
in sessions/**/agents/<agentId>/wire.jsonl as usage.record with fields
usage.inputOther / output / inputCacheRead / inputCacheCreation.

Scan the new home sessions tree recursively to accommodate future nesting changes.

The default root is ~/.kimi-code/sessions. KIMI_CODE_SESSIONS overrides it
with semicolon-separated paths; KIMI_CODE_HOME overrides the entire home directory.
"""
import glob
import json
import os

from . import collector, iter_new_lines, rid, Event

TOOL = "kimi-code"


def _roots():
    env = os.environ.get("KIMI_CODE_SESSIONS", "")
    roots = [p for p in env.split(";") if p.strip()]
    if roots:
        return roots
    home = os.environ.get("KIMI_CODE_HOME", "")
    if not home.strip():
        home = os.path.join(os.path.expanduser("~"), ".kimi-code")
    return [os.path.join(home, "sessions")]


def _iter_wire_files(root):
    """Yield every wire.jsonl recursively under sessions, regardless of nesting depth."""
    for path in glob.glob(os.path.join(root, "**", "wire.jsonl"), recursive=True):
        if os.path.isfile(path):
            yield path


@collector(TOOL)
def collect(ctx):
    found = False
    events = []
    for root in _roots():
        if not os.path.isdir(root):
            continue
        for path in _iter_wire_files(root):
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
        ctx.skip_reason = "未找到新版 Kimi Code 会话目录（~/.kimi-code/sessions）"
    return events
