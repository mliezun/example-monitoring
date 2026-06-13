SELECT
    u.id,
    u.org_id,
    u.username,
    u.password_hash,
    u.created_at,
    o.name AS org_name,
    o.notification_provider,
    o.webhook_url
FROM users u
JOIN organizations o ON o.id = u.org_id
WHERE u.username = ? COLLATE NOCASE;
