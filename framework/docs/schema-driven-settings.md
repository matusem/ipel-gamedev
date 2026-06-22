# Schema-driven settings

The platform renders configuration UIs from **JSON Schema** (draft-07 subset) instead of custom HTML where possible.

## Supported schema subset

- Primitives: `string`, `integer`, `number`, `boolean`
- `enum` and `oneOf` of string literals
- Flat `object` with `properties`, `required`, `additionalProperties: false`
- `array` of primitives (max 32 items)
- Top-level `oneOf: [null, object]` for optional config (tic-tac-toe style)
- Constraints: `minimum`/`maximum`, `minLength`/`maxLength`, `default`, `title`, `description`

Unsupported constructs fall back to a raw JSON editor.

## Validation layers

1. **Lobby `SchemaForm`** — instant client-side subset validation
2. **Server `settings_validation`** — same subset on GraphQL mutations
3. **WASM core** — authoritative for published bots (`validate-settings` export) and game config (`init`)

## Game match config

- `manifest.config_schema` — JSON Schema for lobby config
- `manifest.config_ui_mode` — `generated` (default) or `custom`
- Generated mode uses native `SchemaForm` in the lobby
- Custom mode keeps the `config.html` iframe + postMessage bridge

## Bot settings

- **Published / dev-local bots** export settings via `bot.wit` v2:
  - `default-settings`, `validate-settings`, `settings-schema`, `decide(settings, player-state)`
- **External bots** store `settingsSchemaJson` + `settingsJson` on registration (no WASM)
- **Storage**: global defaults on `bots` row; per-seat overrides on `lobby_seats.bot_settings_json`
- **Effective settings**: seat override → bot registry default → WASM `default-settings`

## GraphQL

- `updateBotSettings(botId, settingsJson, settingsSchemaJson?)`
- `assignBotToSeat(..., settingsJson?)`
- `requestExternalBotSeat(..., settingsJson?)`
- `registerExternalBot(..., settingsSchemaJson?, settingsJson?)`

See [bots-external-protocol.md](bots-external-protocol.md) for external runner delivery of settings over WebSocket.
