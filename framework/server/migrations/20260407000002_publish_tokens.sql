CREATE TABLE IF NOT EXISTS publish_tokens (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL,
    token_hash TEXT NOT NULL UNIQUE,
    expires_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    revoked_at INTEGER,
    FOREIGN KEY (user_id) REFERENCES users (id)
);

CREATE INDEX IF NOT EXISTS idx_publish_tokens_user_id ON publish_tokens (user_id);
CREATE INDEX IF NOT EXISTS idx_publish_tokens_expires_at ON publish_tokens (expires_at);
