# web/ (Rust)

The HTTP service layer for **example-monitoring**, rewritten from Python to Rust on branch `rewrite-in-rust`.

## Unchanged layers

This crate intentionally reuses the rest of the monorepo without modification:

| Layer | Location | Used by Rust web as |
|-------|----------|---------------------|
| Database | `../db/queries/*.sql` | Loaded at runtime |
| Templates | `../templates/` | Rendered with MiniJinja (Jinja2-compatible) |
| Poller | `../tasks/poller.js` | Separate process, same SQLite file |

## Stack

- [Axum](https://github.com/tokio-rs/axum) HTTP server
- [rusqlite](https://github.com/rusqlite/rusqlite) (WAL, busy timeout, foreign keys)
- [MiniJinja](https://github.com/mitsuhiko/minijinja) templates
- Same session/CSRF/password scheme as the Python service (`pbkdf2_sha256`, HMAC-signed cookies)

## Run locally

From the repo root:

```bash
python db/apply_migrations.py
cd web && APP_ROOT=.. DATABASE_PATH=../data/monitoring.db SECRET_KEY=dev-secret cargo run --release
```

Open http://localhost:8000

## Docker

The root `Dockerfile` builds this crate and runs `/app/web/example-monitoring-web`.

```bash
docker compose up --build
docker compose --profile test run --rm e2e
```

All existing Ruby e2e specs pass against the Rust web server without changes to `db/`, `templates/`, `tasks/`, or `tests/`.
