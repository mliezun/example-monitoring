INSERT INTO poll_results (
    site_id,
    status,
    http_status_code,
    response_time_ms,
    attempts_used,
    error_message
)
VALUES (?, ?, ?, ?, ?, ?);
