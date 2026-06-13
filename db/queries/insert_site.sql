INSERT INTO sites (
    org_id,
    name,
    url,
    poll_interval_seconds,
    ok_status_codes,
    max_retries,
    next_poll_at
)
VALUES (?, ?, ?, ?, ?, ?, datetime('now'))
RETURNING id;
