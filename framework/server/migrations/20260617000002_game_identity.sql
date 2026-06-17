-- Per-owner game identity: canonical games table + slug-based live keys.

CREATE TABLE IF NOT EXISTS games (
    id TEXT PRIMARY KEY NOT NULL,
    owner_user_id TEXT NOT NULL,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    current_version TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (owner_user_id) REFERENCES users (id),
    UNIQUE (owner_user_id, name)
);

CREATE INDEX IF NOT EXISTS idx_games_owner ON games (owner_user_id);
CREATE INDEX IF NOT EXISTS idx_games_slug ON games (slug);

ALTER TABLE game_drafts ADD COLUMN slug TEXT;

DROP INDEX IF EXISTS idx_game_drafts_name_version_draft;
CREATE UNIQUE INDEX IF NOT EXISTS idx_game_drafts_owner_name_version_active
    ON game_drafts (owner_user_id, game_name, version)
    WHERE status IN ('ready', 'published');

DROP INDEX IF EXISTS idx_game_reviews_game;
DROP INDEX IF EXISTS idx_game_comments_game;

ALTER TABLE game_storefront RENAME COLUMN game_name TO slug;
ALTER TABLE game_reviews RENAME COLUMN game_name TO slug;
ALTER TABLE game_comments RENAME COLUMN game_name TO slug;

CREATE INDEX IF NOT EXISTS idx_game_reviews_slug ON game_reviews (slug, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_game_comments_slug ON game_comments (slug, created_at DESC);
