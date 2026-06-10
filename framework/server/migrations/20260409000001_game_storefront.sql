CREATE TABLE IF NOT EXISTS game_storefront (
    game_name TEXT PRIMARY KEY NOT NULL,
    owner_user_id TEXT,
    short_tagline TEXT,
    long_description TEXT,
    screenshots_json TEXT NOT NULL DEFAULT '[]',
    patch_notes_json TEXT NOT NULL DEFAULT '[]',
    tags_json TEXT NOT NULL DEFAULT '[]',
    avg_session_mins INTEGER NOT NULL DEFAULT 10,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS game_reviews (
    id TEXT PRIMARY KEY NOT NULL,
    game_name TEXT NOT NULL,
    user_id TEXT NOT NULL,
    display_name TEXT NOT NULL,
    body TEXT NOT NULL,
    aspects_json TEXT NOT NULL,
    helpful_votes INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_game_reviews_game ON game_reviews (game_name, created_at DESC);

CREATE TABLE IF NOT EXISTS game_comments (
    id TEXT PRIMARY KEY NOT NULL,
    game_name TEXT NOT NULL,
    user_id TEXT NOT NULL,
    display_name TEXT NOT NULL,
    body TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_game_comments_game ON game_comments (game_name, created_at DESC);
