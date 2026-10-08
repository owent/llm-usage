# -*- coding: utf-8 -*-
"""LLM usage storage: raw events and daily aggregate cache; prefer cached history, retain one year."""
import os
import sqlite3
import time
from datetime import datetime, timedelta

RETENTION_DAYS = 366

_SCHEMA = """
CREATE TABLE IF NOT EXISTS events(
  request_id TEXT PRIMARY KEY,
  tool       TEXT NOT NULL,
  ts         REAL NOT NULL,
  day        TEXT NOT NULL,
  model      TEXT NOT NULL,
  provider   TEXT NOT NULL,
  input      INTEGER NOT NULL DEFAULT 0,
  output     INTEGER NOT NULL DEFAULT 0,
  cache_read INTEGER NOT NULL DEFAULT 0,
  cache_write INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_events_day ON events(day);
CREATE TABLE IF NOT EXISTS daily(
  day TEXT NOT NULL, tool TEXT NOT NULL, model TEXT NOT NULL,
  requests INTEGER NOT NULL, input INTEGER NOT NULL, output INTEGER NOT NULL,
  cache_read INTEGER NOT NULL, cache_write INTEGER NOT NULL,
  PRIMARY KEY(day, tool, model));
CREATE TABLE IF NOT EXISTS source_state(
  tool TEXT NOT NULL, key TEXT NOT NULL, value TEXT,
  PRIMARY KEY(tool, key));
CREATE TABLE IF NOT EXISTS meta(k TEXT PRIMARY KEY, v TEXT);
"""


def local_day(ts: float) -> str:
    """Date string in the running machine's local timezone."""
    return datetime.fromtimestamp(ts).strftime("%Y-%m-%d")


class Store:
    def __init__(self, path: str):
        os.makedirs(os.path.dirname(path), exist_ok=True)
        self.con = sqlite3.connect(path)
        self.con.executescript(_SCHEMA)
        self.con.commit()

    def close(self):
        self.con.close()

    # ---- Collection state: incremental cursors/file offsets ----
    def state_get(self, tool: str, key: str, default=None):
        row = self.con.execute(
            "SELECT value FROM source_state WHERE tool=? AND key=?", (tool, key)
        ).fetchone()
        return row[0] if row else default

    def state_set(self, tool: str, key: str, value):
        self.con.execute(
            "INSERT OR REPLACE INTO source_state(tool,key,value) VALUES(?,?,?)",
            (tool, key, str(value)),
        )

    # ---- Idempotent event writes ----
    def add_events(self, events) -> list:
        """Insert events and return deduplicated days with newly inserted events."""
        new_days = []
        for e in events:
            day = local_day(e.ts)
            cur = self.con.execute(
                """INSERT OR IGNORE INTO events
                   (request_id,tool,ts,day,model,provider,input,output,cache_read,cache_write)
                   VALUES(?,?,?,?,?,?,?,?,?,?)""",
                (e.request_id, e.tool, e.ts, day, e.model, e.provider,
                 e.input, e.output, e.cache_read, e.cache_write),
            )
            if cur.rowcount == 1:
                new_days.append(day)
        self.con.commit()
        return new_days

    # ---- Daily aggregate cache ----
    def rebuild_days(self, days):
        """Recompute selected dates; historical reads use the cache without revisiting sources."""
        days = sorted(set(days))
        if not days:
            return
        self.con.executemany("DELETE FROM daily WHERE day=?", [(d,) for d in days])
        self.con.execute(
            """INSERT OR REPLACE INTO daily
               SELECT day, tool, model, COUNT(*), SUM(input), SUM(output),
                      SUM(cache_read), SUM(cache_write)
               FROM events WHERE day IN (%s)
               GROUP BY day, tool, model"""
            % ",".join("?" * len(days)),
            days,
        )
        self.con.commit()

    def prune(self, retention_days: int = RETENTION_DAYS):
        cutoff = (datetime.now() - timedelta(days=retention_days)).strftime("%Y-%m-%d")
        self.con.execute("DELETE FROM events WHERE day < ?", (cutoff,))
        self.con.execute("DELETE FROM daily WHERE day < ?", (cutoff,))
        self.con.commit()

    # ---- Queries ----
    def _rows(self, where: str = "", params=()):
        sql = ("SELECT day, tool, model, requests, input, output, cache_read, cache_write "
               "FROM daily " + where + " ORDER BY day, tool, model")
        return self.con.execute(sql, params).fetchall()

    def hourly_rows(self, day: str):
        """Aggregate a date by local hour, tool and model for today's live charts."""
        return self.con.execute(
            """SELECT CAST(strftime('%H', ts, 'unixepoch', 'localtime') AS INTEGER) AS h,
                      tool, model, COUNT(*), SUM(input), SUM(output),
                      SUM(cache_read), SUM(cache_write)
               FROM events WHERE day = ?
               GROUP BY h, tool, model ORDER BY h""",
            (day,),
        ).fetchall()


def hit_rate(cache_read: int, input_tokens: int) -> float:
    denom = cache_read + input_tokens
    return round(cache_read / denom * 100, 2) if denom > 0 else 0.0


def iso_week(day: str) -> str:
    d = datetime.strptime(day, "%Y-%m-%d")
    iso = d.isocalendar()
    return f"{iso[0]}-W{iso[1]:02d}"
