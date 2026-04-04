CREATE TABLE IF NOT EXISTS pregame_lobbies (
    id TEXT PRIMARY KEY NOT NULL,
    owner_user_id TEXT NOT NULL,
    game_type TEXT NOT NULL,
    config TEXT NOT NULL,
    status TEXT NOT NULL,
    game_instance_id TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (owner_user_id) REFERENCES users (id)
);

CREATE TABLE IF NOT EXISTS lobby_seats (
    lobby_id TEXT NOT NULL,
    seat_index INTEGER NOT NULL,
    player_identity TEXT NOT NULL,
    claimed_by_user_id TEXT,
    PRIMARY KEY (lobby_id, seat_index),
    FOREIGN KEY (lobby_id) REFERENCES pregame_lobbies (id) ON DELETE CASCADE,
    FOREIGN KEY (claimed_by_user_id) REFERENCES users (id)
);

CREATE INDEX IF NOT EXISTS idx_pregame_lobbies_status ON pregame_lobbies (status);
CREATE INDEX IF NOT EXISTS idx_lobby_seats_claimed ON lobby_seats (claimed_by_user_id);
