#!/usr/bin/env python3
"""Apply pending SQL migrations in lexical order."""

from __future__ import annotations

import os
import sqlite3
import sys
from pathlib import Path

MIGRATIONS_DIR = Path(__file__).resolve().parent / "migrations"


def connect(db_path: str) -> sqlite3.Connection:
    os.makedirs(os.path.dirname(os.path.abspath(db_path)), exist_ok=True)
    conn = sqlite3.connect(db_path, timeout=30.0)
    conn.row_factory = sqlite3.Row
    conn.execute("PRAGMA foreign_keys = ON")
    conn.execute("PRAGMA journal_mode = WAL")
    conn.execute("PRAGMA busy_timeout = 5000")
    conn.execute("PRAGMA synchronous = NORMAL")
    return conn


def ensure_schema_migrations(conn: sqlite3.Connection) -> None:
    conn.execute(
        """
        CREATE TABLE IF NOT EXISTS schema_migrations (
            filename VARCHAR(255) PRIMARY KEY,
            applied_at VARCHAR(19) NOT NULL DEFAULT (datetime('now'))
        )
        """
    )


def applied_migrations(conn: sqlite3.Connection) -> set[str]:
    rows = conn.execute("SELECT filename FROM schema_migrations").fetchall()
    return {row["filename"] for row in rows}


def apply_migration(conn: sqlite3.Connection, path: Path) -> None:
    sql = path.read_text(encoding="utf-8")
    try:
        conn.execute("BEGIN IMMEDIATE")
        conn.executescript(sql)
        conn.execute(
            "INSERT INTO schema_migrations (filename) VALUES (?)",
            (path.name,),
        )
        conn.commit()
    except Exception:
        conn.rollback()
        raise


def main() -> int:
    db_path = os.environ.get("DATABASE_PATH", "data/monitoring.db")
    conn = connect(db_path)
    ensure_schema_migrations(conn)
    done = applied_migrations(conn)

    pending = sorted(
        p for p in MIGRATIONS_DIR.glob("*.sql") if p.name not in done
    )
    if not pending:
        print("No pending migrations.")
        return 0

    for path in pending:
        print(f"Applying {path.name}...")
        apply_migration(conn, path)
        print(f"Applied {path.name}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
