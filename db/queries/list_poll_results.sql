SELECT
    id,
    site_id,
    checked_at,
    status,
    http_status_code,
    response_time_ms,
    attempts_used,
    error_message
FROM poll_results
WHERE site_id = ?
ORDER BY checked_at DESC
LIMIT ?;
