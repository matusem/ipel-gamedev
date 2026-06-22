-- Bot settings schema + global defaults; per-seat overrides; request snapshots.

ALTER TABLE bots ADD COLUMN settings_schema_json TEXT;
ALTER TABLE bots ADD COLUMN settings_json TEXT;

ALTER TABLE lobby_seats ADD COLUMN bot_settings_json TEXT;

ALTER TABLE lobby_bot_requests ADD COLUMN settings_json TEXT;
