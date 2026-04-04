CREATE TABLE IF NOT EXISTS game_instances (
    id TEXT PRIMARY KEY NOT NULL,
    game_type TEXT NOT NULL,
    config TEXT NOT NULL,
    state TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    updated_at INTEGER NOT NULL
);
