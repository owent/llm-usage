# -*- coding: utf-8 -*-
"""采集器框架。

新增一个工具：在本目录新建 xxx.py，使用 @collector("tool-name") 注册一个
collect(ctx) 生成器函数即可，框架自动发现并调度；删除工具直接删掉对应文件，
或在 collect() 中 yield 空并自行记录跳过原因（返回前设置 ctx.skip_reason）。
"""
import hashlib
import re
from dataclasses import dataclass, field


@dataclass
class Event:
    tool: str
    request_id: str
    ts: float            # epoch 秒
    model: str
    provider: str
    input: int           # 未命中缓存的输入 token
    output: int
    cache_read: int
    cache_write: int


@dataclass
class Ctx:
    store: object                    # store.Store
    refresh: bool = False
    skip_reason: str = ""            # 采集器可填写跳过/失败原因说明


REGISTRY = {}


def collector(name):
    """注册采集器；被装饰函数签名为 collect(ctx) -> Iterable[Event]。"""
    def wrap(fn):
        REGISTRY[name] = fn
        return fn
    return wrap


def iter_new_lines(path, ctx: Ctx, tool: str):
    """从上次偏移增量产出完整新行（jsonl 追加写模型），并更新偏移状态。

    首次见到的文件从 0 开始读（一次性回填历史）；之后只读新增字节。
    文件被截断/轮转时自动从头读。refresh=True 时忽略偏移全量重扫。
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
    if end < 0:                  # 没有完整行，等下一次
        return
    complete = data[:end + 1]
    ctx.store.state_set(tool, path, offset + len(complete))
    for raw in complete.split(b"\n"):
        raw = raw.strip()
        if raw:
            yield raw.decode("utf-8", "replace")


_MODEL_RE = re.compile(r'"model"\s*:\s*"([^"]+)"')


def fast_model(raw_line: str):
    """不解析 JSON 快速提取行内 model 字段（用于需要跟踪"当前模型"的源）。"""
    m = _MODEL_RE.search(raw_line)
    return m.group(1) if m else None


def rid(*parts) -> str:
    """稳定的请求去重 id。"""
    h = hashlib.sha1()
    for p in parts:
        h.update(str(p).encode("utf-8", "replace"))
        h.update(b"\x00")
    return h.hexdigest()
