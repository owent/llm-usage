# -*- coding: utf-8 -*-
"""Collector framework.

To add a tool, create xxx.py here and register a collect(ctx) generator with
@collector("tool-name"). The framework discovers and schedules it automatically.
Remove a tool by deleting its file, or yield nothing from collect() and record
the skip reason by setting ctx.skip_reason before returning.
"""
import hashlib
import re
from dataclasses import dataclass, field


@dataclass
class Event:
    tool: str
    request_id: str
    ts: float            # Epoch seconds.
    model: str
    provider: str
    input: int           # Uncached input tokens.
    output: int
    cache_read: int
    cache_write: int


@dataclass
class Ctx:
    store: object                    # store.Store
    refresh: bool = False
    skip_reason: str = ""            # The collector may record a skip/failure reason.


REGISTRY = {}


def collector(name):
    """Register a collector with signature collect(ctx) -> Iterable[Event]."""
    def wrap(fn):
        REGISTRY[name] = fn
        return fn
    return wrap


def iter_new_lines(path, ctx: Ctx, tool: str):
    """Yield complete new lines incrementally and update offsets (append-only JSONL).

    Read a newly seen file from offset zero for initial historical backfill;
    subsequent reads consume only appended bytes. Restart from the beginning
    after truncation/rotation. refresh=True ignores offsets and rescans everything.
    """
    import os
    try:
        size = os.path.getsize(path)
    except OSError:
        return
    try:
        offset = 0 if ctx.refresh else int(ctx.store.state_get(tool, path, 0) or 0)
    except (ValueError, TypeError):
        offset = 0
    if size < offset:
        offset = 0
    try:
        with open(path, "rb") as f:
            f.seek(offset)
            data = f.read()
    except OSError:
        return
    if not data:
        return
    end = data.rfind(b"\n")
    if end < 0:                  # No complete line yet; wait for the next read.
        return
    complete = data[:end + 1]
    ctx.store.state_set(tool, path, offset + len(complete))
    for raw in complete.split(b"\n"):
        raw = raw.strip()
        if raw:
            yield raw.decode("utf-8", "replace")


_MODEL_RE = re.compile(r'"model"\s*:\s*"([^"]+)"')


def fast_model(raw_line: str):
    """Extract a line model field quickly without JSON parsing for current-model tracking."""
    m = _MODEL_RE.search(raw_line)
    return m.group(1) if m else None


def rid(*parts) -> str:
    """Stable request deduplication ID."""
    h = hashlib.sha1()
    for p in parts:
        h.update(str(p).encode("utf-8", "replace"))
        h.update(b"\x00")
    return h.hexdigest()
