CREATE TABLE IF NOT EXISTS user_roles (
    user_id TEXT NOT NULL,
    role TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (user_id, role),
    FOREIGN KEY (user_id) REFERENCES users (id)
);

CREATE TABLE IF NOT EXISTS game_uploads (
    id TEXT PRIMARY KEY NOT NULL,
    owner_user_id TEXT NOT NULL,
    filename TEXT NOT NULL,
    status TEXT NOT NULL,
    report_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (owner_user_id) REFERENCES users (id)
);

CREATE TABLE IF NOT EXISTS game_drafts (
    id TEXT PRIMARY KEY NOT NULL,
    upload_id TEXT NOT NULL,
    owner_user_id TEXT NOT NULL,
    game_name TEXT NOT NULL,
    display_name TEXT NOT NULL,
    version TEXT NOT NULL,
    status TEXT NOT NULL,
    manifest_json TEXT NOT NULL,
    report_json TEXT NOT NULL,
    storage_path TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    published_at INTEGER,
    FOREIGN KEY (owner_user_id) REFERENCES users (id),
    FOREIGN KEY (upload_id) REFERENCES game_uploads (id)
);

CREATE INDEX IF NOT EXISTS idx_user_roles_role ON user_roles (role);
CREATE INDEX IF NOT EXISTS idx_game_drafts_owner_status ON game_drafts (owner_user_id, status);
CREATE UNIQUE INDEX IF NOT EXISTS idx_game_drafts_name_version_draft
    ON game_drafts (game_name, version)
    WHERE status IN ('ready', 'published');
