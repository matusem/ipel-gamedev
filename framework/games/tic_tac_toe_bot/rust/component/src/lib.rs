//! Component exports for tic-tac-toe bot (`bot.wasm`).

use bot_wasm_host::MyBotHost;
use tic_tac_toe_bot::TicTacToeBot;

type BotWorld = MyBotHost<TicTacToeBot>;

bot_wasm_host::export_game_bot!(BotWorld);
