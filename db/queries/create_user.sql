INSERT INTO users (org_id, username, password_hash)
VALUES (?, ?, ?)
RETURNING id, org_id, username, created_at;
