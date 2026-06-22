//! Component exports for bot (`bot.wasm`).

use bot_wasm_host::MyBotHost;
use __BOT_LOGIC_NAME__::__BOT_LOGIC_NAME__;

type BotWorld = MyBotHost<__BOT_LOGIC_NAME__>;

bot_wasm_host::export_game_bot!(BotWorld);
