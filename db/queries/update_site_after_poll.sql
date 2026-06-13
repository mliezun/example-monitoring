UPDATE sites
SET
    current_status = ?,
    last_checked_at = datetime('now'),
    next_poll_at = datetime('now', '+' || ? || ' seconds')
WHERE id = ?;
