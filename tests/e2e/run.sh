#!/usr/bin/env bash
set -euo pipefail

APP_URL="${APP_URL:-http://web:8000}"

echo "Waiting for ${APP_URL}/health ..."
for i in $(seq 1 30); do
  if curl -fsS "${APP_URL}/health" >/dev/null; then
    echo "App is up."
    break
  fi
  sleep 1
  if [[ "$i" -eq 30 ]]; then
    echo "App did not become ready in time." >&2
    exit 1
  fi
done

bundle exec rspec --format documentation --color
