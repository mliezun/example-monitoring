SELECT
    s.id,
    s.org_id,
    s.name,
    s.url,
    s.poll_interval_seconds,
    s.ok_status_codes,
    s.max_retries,
    s.current_status,
    o.notification_provider,
    o.webhook_url
FROM sites s
JOIN organizations o ON o.id = s.org_id
WHERE s.next_poll_at <= datetime('now')
ORDER BY s.next_poll_at ASC;
