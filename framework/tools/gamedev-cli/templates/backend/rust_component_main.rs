use __LOGIC_NAME__::HelloGame;

type GameWorld = game_wasm_host::MyHost<HelloGame>;
game_wasm_host::export_game_core!(GameWorld);
