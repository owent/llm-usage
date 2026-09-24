# -*- coding: utf-8 -*-
"""copilot 采集器：~/.copilot/session-store.db 与各 session-state/*/session.db 的 assistant_usage_events。

input_tokens 含缓存命中部分，这里拆分为：input = input_tokens - cache_read_tokens。
"""
import glob
import os
import shutil
import sqlite3
import tempfile
from datetime import datetime, timezone

from . import collector, rid, Event

TOOL = "copilot"
HOME = os.path.expanduser("~")
CURSOR_KEY = "cursor_ts"


def _ts(iso) -> float:
    try:
        dt = datetime.fromisoformat(str(iso).replace("Z", "+00:00"))
        if dt.tzinfo is None:
            dt = dt.replace(tzinfo=timezone.utc)
        return dt.timestamp()
    except Exception:
        return 0.0


def _read_db(path, ctx):
    """复制活库后查询增量事件。"""
    tmp = tempfile.mkdtemp(prefix="copilot-db-")
    dst = os.path.join(tmp, "s.db")
    rows = []
    try:
        shutil.copy2(path, dst)
        for ext in ("-wal", "-shm"):
            if os.path.exists(path + ext):
                shutil.copy2(path + ext, dst + ext)
        con = sqlite3.connect(dst)
        try:
            try:
                cursor = 0.0 if ctx.refresh else float(ctx.store.state_get(TOOL, CURSOR_KEY, 0) or 0)
            except (ValueError, TypeError):
                cursor = 0.0
            for r in con.execute(
                "SELECT session_id, turn_index, model, input_tokens, output_tokens, "
                "cache_read_tokens, cache_write_tokens, created_at FROM assistant_usage_events"
            ):
                rows.append((cursor, r))
        finally:
            con.close()
    except sqlite3.Error:
        pass
    finally:
        shutil.rmtree(tmp, ignore_errors=True)
    return rows


@collector(TOOL)
def collect(ctx):
    dbs = [os.path.join(HOME, ".copilot", "session-store.db")]
    dbs += glob.glob(os.path.join(HOME, ".copilot", "session-state", "*", "session.db"))
    events = []
    max_seen = 0.0
    for db in dbs:
        if not os.path.exists(db):
            continue
        for cursor, (sid, turn, model, inp, out, cr, cw, created_at) in _read_db(db, ctx):
            ts = _ts(created_at)
            if ts <= 0:
                continue
            max_seen = max(max_seen, ts)
            if ts < cursor - 3600:          # 增量窗口外，靠 request_id 幂等兜底
                pass
            cr = int(cr or 0)
            events.append(Event(
                tool=TOOL,
                request_id=rid(TOOL, db, sid, turn, model, inp, out, cr, created_at),
                ts=ts,
                model=str(model or "unknown"),
                provider="copilot",
                input=max(0, int(inp or 0) - cr),
                output=int(out or 0),
                cache_read=cr,
                cache_write=int(cw or 0),
            ))
    if max_seen:
        ctx.store.state_set(TOOL, CURSOR_KEY, max_seen)
    return events
