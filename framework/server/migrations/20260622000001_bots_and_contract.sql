-- Bot registry and lobby seat bot assignments

CREATE TABLE IF NOT EXISTS bots (
    id TEXT PRIMARY KEY NOT NULL,
    owner_user_id TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    version TEXT NOT NULL,
    game_slug TEXT NOT NULL,
    game_version TEXT NOT NULL,
    contract_hash TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'published',
    created_at INTEGER NOT NULL,
    FOREIGN KEY (owner_user_id) REFERENCES users (id)
);

CREATE INDEX IF NOT EXISTS idx_bots_game_slug ON bots (game_slug);
CREATE INDEX IF NOT EXISTS idx_bots_contract_hash ON bots (contract_hash);

ALTER TABLE lobby_seats ADD COLUMN bot_id TEXT;
ALTER TABLE lobby_seats ADD COLUMN bot_display_name TEXT;
