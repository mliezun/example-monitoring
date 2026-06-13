# example-monitoring

Proof-of-concept uptime monitor for the blog post [Web services for the AI era](https://mliezun.github.io/). Layers are split so the service can be rewritten (Python → Rust) without touching SQL, templates, or the JS poller.

## Layout

```
db/           migrations + plain .sql queries
web/          sync WSGI app (no Flask/FastAPI)
tasks/        JavaScript background poller
templates/    HTMX + Tailwind CDN
tests/e2e/    human-readable Capybara specs (Cuprite)
```

## Features

- Multi-tenant orgs with one user each; registration enabled
- HTTPS-only monitored URLs; GET checks with configurable OK status codes
- Retries per poll cycle before recording a failed check
- Full poll history in SQLite
- Slack **or** Discord webhook per organization (HTTPS URL)
- Alerts only when status changes (up ↔ down)

## Run with Docker

```bash
cd example-monitoring
docker compose up --build
```

Open http://localhost:8000

Run end-to-end tests (Cuprite inside Compose):

```bash
docker compose --profile test run --rm e2e
```

This runs both UI specs and **background poller** integration specs. Poller tests use [httpbin.org](https://httpbin.org) for controllable HTTP responses (`/status/200`, `/status/503`, etc.) and assert directly against SQLite poll history.

### Test layout

| File | What it covers |
|------|----------------|
| `spec/monitoring_spec.rb` | UI flows (register, sites, settings) |
| `spec/poller_spec.rb` | Background worker + httpbin + DB assertions |
| `spec/support/monitoring_helpers.rb` | Readable helper verbs (`add_monitored_site`, `wait_until`) |

Poller specs are written to read as short stories: register → add site → wait until DB shows result → assert facts. Helpers hide Capybara and SQL noise.

## Run locally

```bash
python db/apply_migrations.py
pip install -r requirements.txt
npm install
DATABASE_PATH=data/monitoring.db SECRET_KEY=dev-secret python -m web.main &
DATABASE_PATH=data/monitoring.db node tasks/poller.js
```

## Migration naming

New files in `db/migrations/` use:

```
{migration_name}_{YYYYMMDDHHMMSS}_{random}.sql
```

Example: `add_foo_column_20260613143000_b4c9d1.sql`

Apply with `python db/apply_migrations.py` (tracks applied files in `schema_migrations`).

## Environment

| Variable | Default | Purpose |
|----------|---------|---------|
| `DATABASE_PATH` | `data/monitoring.db` | SQLite file |
| `SECRET_KEY` | dev default | Session signing |
| `POLL_TICK_MS` | `5000` | Poller loop interval |
| `COOKIE_SECURE` | `0` | Set `1` behind HTTPS |

## Out of scope (POC)

TLS termination, rate limiting, public status pages, email alerts, OAuth, sample seed data.
