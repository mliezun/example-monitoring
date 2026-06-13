SELECT
    id,
    org_id,
    name,
    url,
    poll_interval_seconds,
    ok_status_codes,
    max_retries,
    current_status,
    last_checked_at,
    next_poll_at,
    created_at
FROM sites
WHERE id = ? AND org_id = ?;
