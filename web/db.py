"""Production-oriented SQLite access for the WSGI service."""

from __future__ import annotations

import os
import sqlite3
import threading
from contextlib import contextmanager
from functools import lru_cache
from pathlib import Path
from typing import Any, Iterator

QUERIES_DIR = Path(__file__).resolve().parents[1] / "db" / "queries"
_local = threading.local()


def database_path() -> str:
    return os.environ.get("DATABASE_PATH", "data/monitoring.db")


def connect(db_path: str | None = None) -> sqlite3.Connection:
    path = db_path or database_path()
    os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
    conn = sqlite3.connect(
        path,
        timeout=30.0,
        isolation_level=None,
        check_same_thread=False,
    )
    conn.row_factory = sqlite3.Row
    conn.execute("PRAGMA foreign_keys = ON")
    conn.execute("PRAGMA journal_mode = WAL")
    conn.execute("PRAGMA busy_timeout = 5000")
    conn.execute("PRAGMA synchronous = NORMAL")
    return conn


def _connection() -> sqlite3.Connection:
    conn = getattr(_local, "conn", None)
    if conn is None:
        conn = connect()
        _local.conn = conn
    return conn


@contextmanager
def transaction() -> Iterator[sqlite3.Connection]:
    conn = _connection()
    conn.execute("BEGIN IMMEDIATE")
    try:
        yield conn
        conn.execute("COMMIT")
    except Exception:
        conn.execute("ROLLBACK")
        raise


@lru_cache(maxsize=128)
def load_query(name: str) -> str:
    path = QUERIES_DIR / name
    if not path.is_file():
        raise FileNotFoundError(f"Missing query file: {name}")
    return path.read_text(encoding="utf-8").strip()


def fetchone(query_name: str, params: tuple[Any, ...] = ()) -> sqlite3.Row | None:
    sql = load_query(query_name)
    cur = _connection().execute(sql, params)
    return cur.fetchone()


def fetchall(query_name: str, params: tuple[Any, ...] = ()) -> list[sqlite3.Row]:
    sql = load_query(query_name)
    cur = _connection().execute(sql, params)
    return cur.fetchall()


def execute(query_name: str, params: tuple[Any, ...] = ()) -> sqlite3.Cursor:
    sql = load_query(query_name)
    return _connection().execute(sql, params)
