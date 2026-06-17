CREATE TABLE IF NOT EXISTS friendships (
    user_a TEXT NOT NULL,
    user_b TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    requested_by TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    accepted_at INTEGER,
    PRIMARY KEY (user_a, user_b),
    FOREIGN KEY (user_a) REFERENCES users (id) ON DELETE CASCADE,
    FOREIGN KEY (user_b) REFERENCES users (id) ON DELETE CASCADE,
    FOREIGN KEY (requested_by) REFERENCES users (id)
);

CREATE INDEX IF NOT EXISTS idx_friendships_user_a ON friendships(user_a);
CREATE INDEX IF NOT EXISTS idx_friendships_user_b ON friendships(user_b);

CREATE TABLE IF NOT EXISTS friend_activity (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    target TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_friend_activity_user ON friend_activity(user_id, created_at);
