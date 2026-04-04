-- Rebuild pregame_lobbies with nullable config; migrate statuses; partial unique seat constraint; lobby chat.
PRAGMA foreign_keys = OFF;

CREATE TABLE lobby_seats_backup AS SELECT * FROM lobby_seats;

DROP TABLE lobby_seats;

CREATE TABLE pregame_lobbies_new (
    id TEXT PRIMARY KEY NOT NULL,
    owner_user_id TEXT NOT NULL,
    game_type TEXT NOT NULL,
    config TEXT,
    status TEXT NOT NULL,
    game_instance_id TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (owner_user_id) REFERENCES users (id)
);

INSERT INTO pregame_lobbies_new (id, owner_user_id, game_type, config, status, game_instance_id, created_at, updated_at)
SELECT
    id,
    owner_user_id,
    game_type,
    config,
    CASE status
        WHEN 'open' THEN 'waiting'
        WHEN 'started' THEN 'in_game'
        ELSE status
    END,
    game_instance_id,
    created_at,
    updated_at
FROM pregame_lobbies;

DROP TABLE pregame_lobbies;

ALTER TABLE pregame_lobbies_new RENAME TO pregame_lobbies;

CREATE TABLE lobby_seats (
    lobby_id TEXT NOT NULL,
    seat_index INTEGER NOT NULL,
    player_identity TEXT NOT NULL,
    claimed_by_user_id TEXT,
    PRIMARY KEY (lobby_id, seat_index),
    FOREIGN KEY (lobby_id) REFERENCES pregame_lobbies (id) ON DELETE CASCADE,
    FOREIGN KEY (claimed_by_user_id) REFERENCES users (id)
);

INSERT INTO lobby_seats SELECT * FROM lobby_seats_backup;

DROP TABLE lobby_seats_backup;

CREATE UNIQUE INDEX IF NOT EXISTS idx_lobby_one_seat_per_user
ON lobby_seats (lobby_id, claimed_by_user_id)
WHERE claimed_by_user_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS lobby_messages (
    id TEXT PRIMARY KEY NOT NULL,
    lobby_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    body TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (lobby_id) REFERENCES pregame_lobbies (id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users (id)
);

CREATE INDEX IF NOT EXISTS idx_lobby_messages_lobby ON lobby_messages (lobby_id, created_at);

CREATE INDEX IF NOT EXISTS idx_pregame_lobbies_status ON pregame_lobbies (status);
CREATE INDEX IF NOT EXISTS idx_lobby_seats_claimed ON lobby_seats (claimed_by_user_id);

PRAGMA foreign_keys = ON;
