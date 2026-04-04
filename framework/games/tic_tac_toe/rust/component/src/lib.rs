//! Component exports for tic-tac-toe (`logic.wasm`).

use game_wasm_host::MyHost;
use tic_tac_toe::TicTacToe;

type TicTacToeWorld = MyHost<TicTacToe>;

game_wasm_host::export_game_core!(TicTacToeWorld);
