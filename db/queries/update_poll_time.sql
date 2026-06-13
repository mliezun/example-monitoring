UPDATE sites
SET next_poll_at = datetime('now')
WHERE id = ?;
