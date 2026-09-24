# -*- coding: utf-8 -*-
"""新版 Kimi Code 采集器（v2.x，桌面端 / CLI 独立版）。

新版 Kimi Code 的 home 目录从 kimi-code 内核共享目录迁到了 `~/.kimi-code`
（如 C:\\Users\\<user>\\.kimi-code），但会话事件仍写在
`sessions/**/agents/<agentId>/wire.jsonl` 中，记录格式仍为 `usage.record`
（字段：usage.inputOther / output / inputCacheRead / inputCacheCreation）。

本采集器递归扫描新 home 的 sessions 树，兼容未来层级变化。

默认搜索 ~/.kimi-code/sessions，可用环境变量 KIMI_CODE_SESSIONS 覆盖
（多个路径用 ; 分隔）；KIMI_CODE_HOME 可整体覆盖 home 目录。
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
    """递归产出 sessions 树下所有 wire.jsonl（兼容任意层级）。"""
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
