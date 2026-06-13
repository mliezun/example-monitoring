-- Organizations own monitored sites and notification settings.
CREATE TABLE organizations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name VARCHAR(255) NOT NULL,
    notification_provider VARCHAR(7) CHECK (notification_provider IN ('slack', 'discord')),
    webhook_url VARCHAR(2048),
    created_at VARCHAR(19) NOT NULL DEFAULT (datetime('now'))
);

-- One user per organization.
CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    org_id INTEGER NOT NULL UNIQUE REFERENCES organizations(id) ON DELETE CASCADE,
    username VARCHAR(64) NOT NULL UNIQUE COLLATE NOCASE,
    password_hash VARCHAR(128) NOT NULL,
    created_at VARCHAR(19) NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_users_username ON users(username);

-- Monitored HTTPS endpoints.
CREATE TABLE sites (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    org_id INTEGER NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    url VARCHAR(2048) NOT NULL,
    poll_interval_seconds INTEGER NOT NULL DEFAULT 60 CHECK (poll_interval_seconds >= 30),
    ok_status_codes VARCHAR(128) NOT NULL DEFAULT '200',
    max_retries INTEGER NOT NULL DEFAULT 3 CHECK (max_retries >= 1),
    current_status VARCHAR(7) NOT NULL DEFAULT 'unknown' CHECK (current_status IN ('up', 'down', 'unknown')),
    last_checked_at VARCHAR(19),
    next_poll_at VARCHAR(19) NOT NULL DEFAULT (datetime('now')),
    created_at VARCHAR(19) NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_sites_org_id ON sites(org_id);
CREATE INDEX idx_sites_next_poll_at ON sites(next_poll_at);

-- Full poll history; one row per poll cycle.
CREATE TABLE poll_results (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    site_id INTEGER NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    checked_at VARCHAR(19) NOT NULL DEFAULT (datetime('now')),
    status VARCHAR(4) NOT NULL CHECK (status IN ('up', 'down')),
    http_status_code INTEGER,
    response_time_ms INTEGER,
    attempts_used INTEGER NOT NULL DEFAULT 1,
    error_message VARCHAR(512)
);

CREATE INDEX idx_poll_results_site_id_checked_at ON poll_results(site_id, checked_at DESC);
