# -*- coding: utf-8 -*-
"""Kilo Code collector: assistant message tokens in ~/.local/share/kilo/kilo.db."""
import json
import os
import shutil
import sqlite3
import tempfile

from . import collector, Event

TOOL = "kilo-code"
HOME = os.path.expanduser("~")
SRC = os.path.join(HOME, ".local", "share", "kilo", "kilo.db")
CURSOR_KEY = "cursor_ms"


@collector(TOOL)
def collect(ctx):
    if not os.path.exists(SRC):
        ctx.skip_reason = "未找到 kilo.db"
        return []
    # Copy the live database, including WAL, before reading to avoid locks and dirty reads.
    tmp = tempfile.mkdtemp(prefix="kilo-db-")
    dst = os.path.join(tmp, "kilo.db")
    try:
        shutil.copy2(SRC, dst)
        for ext in ("-wal", "-shm"):
            if os.path.exists(SRC + ext):
                shutil.copy2(SRC + ext, dst + ext)
        con = sqlite3.connect(dst)
        try:
            try:
                cursor = 0 if ctx.refresh else int(ctx.store.state_get(TOOL, CURSOR_KEY, 0) or 0)
            except (ValueError, TypeError):
                cursor = 0
            # Rewind the window by one hour and deduplicate with request_id.
            rows = con.execute(
                "SELECT id, data FROM message WHERE time_created >= ?",
                (max(0, cursor - 3600_000),),
            ).fetchall()
        finally:
            con.close()
    finally:
        shutil.rmtree(tmp, ignore_errors=True)

    events = []
    max_seen = cursor
    for mid, data in rows:
        try:
            d = json.loads(data)
        except Exception:
            continue
        if d.get("role") != "assistant":
            continue
        tokens = d.get("tokens")
        t = (d.get("time") or {}).get("created")
        if not tokens or not t:
            continue
        cache = tokens.get("cache") or {}
        max_seen = max(max_seen, int(t))
        events.append(Event(
            tool=TOOL,
            request_id=f"kilo:{mid}",
            ts=int(t) / 1000.0,
            model=str(d.get("modelID") or "unknown"),
            provider=str(d.get("providerID") or ""),
            input=int(tokens.get("input") or 0),
            output=int(tokens.get("output") or 0),
            cache_read=int(cache.get("read") or 0),
            cache_write=int(cache.get("write") or 0),
        ))
    ctx.store.state_set(TOOL, CURSOR_KEY, max_seen)
    return events
