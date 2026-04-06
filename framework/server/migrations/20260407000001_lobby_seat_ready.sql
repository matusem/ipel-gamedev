-- Per-seat ready flag before the owner can start the game.
ALTER TABLE lobby_seats ADD COLUMN ready INTEGER NOT NULL DEFAULT 0;
