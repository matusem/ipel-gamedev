-- Game session duration
ALTER TABLE game_instances ADD COLUMN started_at INTEGER;
UPDATE game_instances SET started_at = updated_at WHERE started_at IS NULL;

-- Storefront catalog fields
ALTER TABLE game_storefront ADD COLUMN featured INTEGER NOT NULL DEFAULT 0;
ALTER TABLE game_storefront ADD COLUMN creator_display_name TEXT;

-- KPI trend history
CREATE TABLE IF NOT EXISTS platform_metrics_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    captured_at INTEGER NOT NULL,
    active_lobbies INTEGER NOT NULL,
    published_game_types INTEGER NOT NULL,
    finished_games24h INTEGER NOT NULL,
    active_sessions INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_platform_metrics_captured
    ON platform_metrics_snapshots (captured_at DESC);

-- Review helpful votes (one vote per user per review)
CREATE TABLE IF NOT EXISTS game_review_votes (
    review_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (review_id, user_id)
);

-- Auth sessions (bearer token -> user)
CREATE TABLE IF NOT EXISTS auth_sessions (
    token_hash TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_auth_sessions_user ON auth_sessions (user_id);
CREATE INDEX IF NOT EXISTS idx_auth_sessions_expires ON auth_sessions (expires_at);
