INSERT INTO organizations (name)
VALUES (?)
RETURNING id, name, notification_provider, webhook_url, created_at;
