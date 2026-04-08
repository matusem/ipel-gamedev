use my_game_logic::TicTacToe;

type GameWorld = game_wasm_host::MyHost<TicTacToe>;
game_wasm_host::export_game_core!(GameWorld);
