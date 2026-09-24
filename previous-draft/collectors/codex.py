# -*- coding: utf-8 -*-
"""codex 采集器：~/.codex/sessions/**/rollout-*.jsonl 的 token_usage_record。

input_tokens 含缓存命中部分，这里拆分为：input = input_tokens - cached_input_tokens，
cache_read = cached_input_tokens，cache_write = cache_write_input_tokens。

模型名跟踪：usage 记录本身不带模型，模型出现在 event_msg(thread_settings_applied)
等行里。增量读取时新数据块可能只含 usage 行，因此：
1. 每个文件的"最近已知模型"持久化到 source_state（model:<path>），跨增量块保留；
2. 仍解析不出模型的事件，按时间戳回扫全文件，取最近一条不晚于该事件的模型行补齐。
"""
import glob
import json
import os
from datetime import datetime, timezone

from . import collector, iter_new_lines, fast_model, rid, Event

TOOL = "codex"
HOME = os.path.expanduser("~")
MODEL_KEY = "model:"


def _ts(iso) -> float:
    try:
        dt = datetime.fromisoformat(str(iso).replace("Z", "+00:00"))
        if dt.tzinfo is None:
            dt = dt.replace(tzinfo=timezone.utc)
        return dt.timestamp()
    except Exception:
        return 0.0


def _models_by_ts(path):
    """全文件扫描模型行，返回按文件顺序的 (ts, model) 列表（跳过 usage 行）。"""
    out = []
    try:
        with open(path, encoding="utf-8", errors="replace") as f:
            for line in f:
                if '"model"' not in line or '"token_usage_record"' in line:
                    continue
                m = fast_model(line)
                if not m:
                    continue
                try:
                    obj = json.loads(line)
                except Exception:
                    continue
                ts = _ts(obj.get("timestamp"))
                out.append((ts, m))
    except OSError:
        pass
    return out


def _resolve_model(pairs, ts):
    """取最近一条 ts_model <= ts 的模型行；事件早于首条模型行时用首条。"""
    if not pairs:
        return None
    best = pairs[0][1]
    for pts, m in pairs:
        if pts and ts and pts > ts:
            break
        best = m
    return best


@collector(TOOL)
def collect(ctx):
    events = []
    for path in glob.glob(os.path.join(HOME, ".codex", "sessions", "**", "rollout-*.jsonl"),
                          recursive=True):
        state_key = MODEL_KEY + path
        current_model = None
        if not ctx.refresh:
            try:
                current_model = ctx.store.state_get(TOOL, state_key) or None
            except Exception:
                current_model = None
        pending = []  # 未解析出模型的事件下标
        for raw in iter_new_lines(path, ctx, TOOL):
            if '"token_usage_record"' not in raw and '"model"' not in raw:
                continue
            if '"model"' in raw and '"token_usage_record"' not in raw:
                m = fast_model(raw)
                if m:
                    current_model = m
                    ctx.store.state_set(TOOL, state_key, m)
                continue
            try:
                obj = json.loads(raw)
            except Exception:
                continue
            if obj.get("type") != "token_usage_record":
                continue
            payload = obj.get("payload") or {}
            u = payload.get("usage")
            if not u:
                continue
            cached = int(u.get("cached_input_tokens") or 0)
            total_in = int(u.get("input_tokens") or 0)
            ts = _ts(obj.get("timestamp"))
            if not ts:
                continue
            events.append(Event(
                tool=TOOL,
                request_id=rid(TOOL, path, payload.get("response_id")),
                ts=ts,
                model=current_model or "unknown",
                provider="",
                input=max(0, total_in - cached),
                output=int(u.get("output_tokens") or 0),
                cache_read=cached,
                cache_write=int(u.get("cache_write_input_tokens") or 0),
            ))
            if not current_model:
                pending.append(len(events) - 1)
        if pending:
            pairs = _models_by_ts(path)
            for idx in pending:
                m = _resolve_model(pairs, events[idx].ts)
                if m:
                    events[idx].model = m
    return events
