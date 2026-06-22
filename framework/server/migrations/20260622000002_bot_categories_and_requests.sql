-- Bot categories, external/dev-local seats, seat requests, and API keys

ALTER TABLE bots ADD COLUMN category TEXT NOT NULL DEFAULT 'published';
ALTER TABLE bots ADD COLUMN avatar_seed TEXT;
ALTER TABLE bots ADD COLUMN avatar_url TEXT;

ALTER TABLE lobby_seats ADD COLUMN external_bot INTEGER NOT NULL DEFAULT 0;
ALTER TABLE lobby_seats ADD COLUMN external_bot_token TEXT;
ALTER TABLE lobby_seats ADD COLUMN external_bot_category TEXT;
ALTER TABLE lobby_seats ADD COLUMN bot_avatar_seed TEXT;
ALTER TABLE lobby_seats ADD COLUMN bot_avatar_url TEXT;

CREATE TABLE IF NOT EXISTS lobby_bot_requests (
    id TEXT PRIMARY KEY NOT NULL,
    lobby_id TEXT NOT NULL,
    category TEXT NOT NULL,
    requested_by_user_id TEXT NOT NULL,
    requested_by_bot_id TEXT,
    bot_identity_id TEXT NOT NULL,
    label TEXT NOT NULL,
    avatar_seed TEXT,
    avatar_url TEXT,
    game_slug TEXT NOT NULL,
    contract_hash TEXT NOT NULL,
    desired_seat_index INTEGER,
    status TEXT NOT NULL DEFAULT 'pending',
    seat_index INTEGER,
    connect_token TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (lobby_id) REFERENCES pregame_lobbies (id),
    FOREIGN KEY (requested_by_user_id) REFERENCES users (id)
);

CREATE INDEX IF NOT EXISTS idx_lobby_bot_requests_lobby ON lobby_bot_requests (lobby_id, status);

CREATE TABLE IF NOT EXISTS bot_api_keys (
    id TEXT PRIMARY KEY NOT NULL,
    bot_id TEXT NOT NULL,
    owner_user_id TEXT NOT NULL,
    token_hash TEXT NOT NULL UNIQUE,
    prefix TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    last_used_at INTEGER,
    revoked_at INTEGER,
    FOREIGN KEY (bot_id) REFERENCES bots (id),
    FOREIGN KEY (owner_user_id) REFERENCES users (id)
);

CREATE INDEX IF NOT EXISTS idx_bot_api_keys_bot ON bot_api_keys (bot_id);
