-- Persist finished game outcomes, scores, seat snapshot; link optional lobby.

ALTER TABLE game_instances ADD COLUMN lobby_id TEXT;
ALTER TABLE game_instances ADD COLUMN finished_at INTEGER;
ALTER TABLE game_instances ADD COLUMN result_json TEXT;
ALTER TABLE game_instances ADD COLUMN player_scores_json TEXT;
ALTER TABLE game_instances ADD COLUMN seats_snapshot_json TEXT;
