UPDATE organizations
SET
    notification_provider = ?,
    webhook_url = ?
WHERE id = ?;
