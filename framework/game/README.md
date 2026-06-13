# `game` Crate Architecture

This crate defines the framework's core contract for turn-based multiplayer game logic. A game implementation does not talk to the server, WebSocket layer, or lobby directly. Instead, it implements `GameCore` and a few supporting traits, and the framework hosts that implementation inside `logic.wasm`.

The important design idea is that the framework keeps **two views of the same match**:

- `State`: the authoritative full game state.
- `PlayerState`: one per player, containing only what that player is allowed to know and what is needed to validate that player's future actions.

That split is what lets the framework support hidden information games without exposing secret state to every client.

## Mental model

Think of the game runtime as an event pipeline:

1. The lobby produces a JSON config.
2. The framework validates and initializes the game.
3. Each player gets a private `PlayerState`.
4. A client sends one action.
5. The game mutates authoritative `State` and emits game events.
6. The framework derives per-player events from those game events.
7. Each player's `PlayerState` is incrementally updated from the events visible to that player.

The framework then sends only that player's derived events over the `/game` WebSocket.

## Core types and responsibilities

### `Config<GameT>`

`Config` is the input contract between the lobby and the game logic.

- `validate()` checks whether the requested setup is legal.
- `get_players()` returns the exact player identities that will exist in the match.

Those player identities become stable seat identifiers across the whole framework. They are serialized into the initial WASM game snapshot and are later used by the server and WebSocket endpoint to route actions/events.

Rule of thumb:

- Put setup choices here.
- Do not put mutable match state here.
- Return deterministic players from `get_players()`.

### `Action<GameT>`

`Action` is the command a player sends from the client. It should represent intent, not already-applied state changes.

Examples:

- good: "place mark at row 1 col 2"
- good: "move piece along this path"
- bad: "set board to this new board"

The framework serializes/deserializes this type over JSON when using the current server path.

### `PlayerState<GameT>`

`PlayerState` is the per-player projection of the match. It has three jobs:

1. identify which player it belongs to via `get_player()`
2. validate whether that player may perform an action via `can_take_action()`
3. update itself incrementally from visible player events via `apply_event()`

This is the most important concept for implementers.

`PlayerState` is **not** just a convenience cache. In the current framework flow, the host passes the acting player's `PlayerState` into `GameCore::apply_action()`, and that state is the first validation gate for illegal moves. After a successful move, the host updates every stored `PlayerState` by replaying only that player's visible `PlayerEvent`s.

That means:

- if a rule is required to reject an action, `can_take_action()` should know it
- if a player needs information later to validate future actions, that information must live in `PlayerState`
- if some data is secret from a player, do not leak it through `PlayerEvent` or `PlayerState`

### `FullState<GameT>`

`FullState` is the authoritative persisted match snapshot:

- `config`
- `state`
- `actions_made`

The framework serializes this whole value into the WASM host snapshot and updates it after every action.

### Event layers

There are three related event enums:

- `InGameEvent`
  - `PlayerAction`
  - domain `Event`
- `Event`
  - `InGameEvent`
  - `GameOver`
- `PlayerEvent`
  - visible `PlayerEvent`
  - `GameOver`

This layering exists so the framework can:

1. remember that a player action happened
2. include extra domain events emitted by `take_action()`
3. append a terminal `GameOver`
4. derive a different visible stream for each player

## `GameCore` lifecycle

### 1. Initialization

Implement:

- `init(config) -> State`

The default `try_init()` already:

1. calls `config.validate()`
2. clones the config
3. creates `FullState { config, state, actions_made: [] }`

The WASM host then creates one `PlayerState` for every player returned by `config.get_players()`.

### 2. Action processing

Implement:

- `take_action(state, player_action) -> Vec<Event>`
- `check_game_over(state) -> Option<Result>`

The default `apply_action()` in `game/src/lib.rs` performs the runtime flow:

1. `player_state.can_take_action(&action)`
2. append action to `actions_made`
3. call `take_action()` to mutate `State`
4. prepend a `PlayerAction` event
5. append any game-specific events returned by `take_action()`
6. append `GameOver(result)` if `check_game_over()` returns one
7. convert that event list into a per-player event map

Important consequence: `take_action()` is written as if the action is already valid for the current player and current state. You can still code defensively, but the intended place for user-facing move rejection is `can_take_action()`.

### 3. Visibility derivation

Implement:

- `derive_player_event(state, player, event) -> Option<PlayerEvent>`
- `derive_player_result(state, player, result) -> PlayerResult`

This is the privacy boundary of the architecture.

Use `derive_player_event()` to decide what each player is allowed to observe:

- fully public games can often expose every action to everyone
- hidden-information games can emit different payloads per player
- invisible events should return `None`

`derive_player_result()` does the same for the terminal outcome. The authoritative `Result` may be richer than what an individual player should see.

### 4. Spectator projection (v1: single public stream)

All observers share one derived view. Implement:

- `SpectatorState` — incremental public state (`init`, `apply_event`)
- `derive_spectator_event(state, event) -> Option<SpectatorEvent>`
- `derive_spectator_result(state, result) -> SpectatorResult`

`apply_action()` returns `ActionApplicationResult` with both `player_events` (per seat) and `spectator_events` (one list broadcast to every spectator socket). Spectators never send actions.

For open-information games (tic-tac-toe), mirror board moves in `derive_spectator_event` and keep `SpectatorState` in sync with public fields only. For hidden-information games, return public ticks in spectator events and never copy player-only secrets (see `game/src/spectator_tests.rs`).

### 4. Scoring

Implement:

- `scores_at_end(result) -> Vec<(Player, f64)>`

This is the canonical numeric interpretation of a finished game outcome. Follow the documented convention:

- win = `1.0`
- loss = `0.0`
- draw = equal split

Even though the current runtime path is mostly driven by `GameOver` player events, this method should still be implemented consistently because it is part of the core contract for rankings and analytics.

## How the framework uses this crate

The runtime path outside this crate is:

1. A game-specific Rust crate implements `GameCore`.
2. That crate exports the implementation through `game-wasm-host`.
3. `cargo component build --release` produces `logic.wasm`.
4. The game bundle is uploaded or copied into `GAMES_DIR`.
5. The server validates that `logic.wasm` is a WebAssembly Component and can instantiate the `GameCore` world.
6. When a lobby starts a match, the server calls WASM `init`.
7. The returned `Game` snapshot contains:
   - serialized `FullState`
   - serialized `PlayerState` for each player
8. Each `/game` WebSocket connection registers one player identity.
9. The client sends raw action JSON.
10. The server calls WASM `take_action`.
11. The host updates authoritative state and every player's private state.
12. The server sends each player only that player's serialized `PlayerEvent`s.

## Package-level expectations for an implementer

To make a playable game work in this framework, the implementing agent usually needs all of the following:

### 1. Rust logic crate

Implement the traits from `game`:

- `Config`
- `Action`
- `PlayerState`
- `GameCore`

Reference examples:

- `games/tic_tac_toe/rust/logic/src/lib.rs`
- `games/checkers/rust/logic/src/lib.rs`

### 2. WASM host export

The component crate should expose the game using `game-wasm-host`, following the pattern used by the scaffolded games:

```rust
use game_wasm_host::MyHost;

type MyGameWorld = MyHost<MyGame>;
game_wasm_host::export_game_core!(MyGameWorld);
```

That host is what translates serialized framework calls into your trait methods.

### 3. Bundle manifest

The published game directory needs a `manifest.json` with at least:

- `name`
- `display_name`
- `version`
- `min_players`
- `max_players`
- optional `description`
- optional `config_entry`, `result_entry`, `about_entry`
- optional `config_schema`

The authoritative manifest shape currently lives in `server/src/game_registry.rs`.

### 4. Client assets

The uploaded/published game bundle is expected to contain:

- `client/index.html`
- `client/config.html`
- `client/result.html`
- `client/about.html`

The lobby loads `config.html` in an iframe. That iframe does not save config directly; it only previews config back to the lobby via `postMessage`.

### 5. Browser protocol

For `config.html`:

- lobby sends schema via `upjs-gdd-game-config-schema`
- lobby sends current saved config via `upjs-gdd-game-config-state`
- iframe sends previews via `upjs-gdd-game-config`
- lobby replies with validation via `upjs-gdd-game-config-result`

For `index.html`:

- connect to `/game?id=...&player=...`
- first inbound frame is the initial player-visible state
- later inbound frames are player-visible events
- outbound frames are only the serialized action, not `{ player, action }`

`packages/game-sdk/src/game-client.ts` and `packages/game-sdk/src/lobby-config-bridge.ts` are the best references for this protocol.

## Implementation guidance for the next agent

When implementing a new game against this architecture, keep these boundaries strict:

- `State` is the source of truth.
- `PlayerState` is the player's private, incrementally updated view.
- `Action` is intent.
- `Event` is domain-level fallout from an accepted action.
- `PlayerEvent` is the filtered, client-visible stream.
- `Result` is authoritative.
- `PlayerResult` is the player-facing terminal interpretation.

If the game has no hidden information, you can often keep `PlayerState` close to `State` and derive the same event for every player. If the game has hidden information, design `PlayerState` and `derive_player_event()` first, because they determine what the framework can safely send to clients.

## Recommended implementation order

1. Define `Player`, `Action`, `Result`, and `PlayerResult`.
2. Define `Config` and make `validate()` strict.
3. Define authoritative `State`.
4. Define `PlayerState` based on exactly what each player may know.
5. Implement `init()`.
6. Implement `can_take_action()` before `take_action()`.
7. Implement `take_action()` and `check_game_over()`.
8. Implement `derive_player_event()` and `derive_player_result()`.
9. Add focused tests around illegal moves, hidden information, and terminal outcomes.
10. Export through `game-wasm-host` and build `logic.wasm`.

## Practical rule of thumb

If you are unsure where some piece of data belongs:

- put it in `State` if it is part of the real game truth
- put it in `PlayerState` if a player needs a local/private projection of that truth
- emit it as `Event` if it happens because of a move
- emit it as `PlayerEvent` only if that specific player is allowed to see it

That mental model matches how `game/src/lib.rs`, `game-wasm-host`, and the server runtime currently cooperate.
