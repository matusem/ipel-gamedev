//! Component exports for checkers (`logic.wasm`).

use checkers::Checkers;
use game_wasm_host::MyHost;

type CheckersWorld = MyHost<Checkers>;

game_wasm_host::export_game_core!(CheckersWorld);
