use __LOGIC_NAME__::TicTacToe;

type GameWorld = game_wasm_host::MyHost<TicTacToe>;
game_wasm_host::export_game_core!(GameWorld);
