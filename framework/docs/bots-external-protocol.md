# External bot protocol

Language-agnostic guide for **external** bots (category `external`): long-lived agents that run outside the platform, authenticate with a **bot API key**, and play via the same WebSocket contract as dev-local bots.

Published bots (WASM on the server) and dev-local bots (`gamedev bot-run`) are documented in the main [README](../README.md); this document focuses on the external path.

## Prerequisites

1. A game published on the platform with a `contract.json` (JSON Schema type link).
2. An **external bot** registered in the developer area (or via GraphQL `registerExternalBot`).
3. A **bot API key** (`createBotApiKey`) — shown once as `gbk_<uuid>`.

## Type contract

Fetch the canonical schema:

```graphql
query {
  gameContract(slug: "my-game") {
    contractHash
    schemaJson
  }
}
```

Generate client types from `schemaJson` in your language (JSON Schema → TypeScript, Python, etc.). Your bot must implement:

```text
decide(PlayerState) -> Option<Action>
```

using the `Player`, `Action`, and `PlayerState` slots defined in the contract.

## Authentication

Send the API key on every GraphQL request:

```http
Authorization: Bearer gbk_<secret>
```

The key resolves to a bot principal (`bot_id`, `game_slug`, `contract_hash`). It does **not** replace the per-match **connect token** (see below).

## Seat request / approve handshake

### 1. Request a seat

```graphql
mutation RequestSeat(
  $lobbyId: ID!
  $category: String!
  $label: String!
  $contractHash: String!
  $desiredSeatIndex: Int
) {
  requestExternalBotSeat(
    lobbyId: $lobbyId
    category: "external"
    label: $label
    contractHash: $contractHash
    desiredSeatIndex: $desiredSeatIndex
  ) {
    requestId
    connectToken
  }
}
```

- `category` must be `"external"` when using an API key.
- `contractHash` must match the lobby's game contract.
- Returns `requestId` and `connectToken` (`bct_<uuid>`).

### 2. Poll until approved

```graphql
query BotRequest($requestId: ID!) {
  botRequest(requestId: $requestId) {
    status
    seatIndex
    connectToken
    lobbyId
  }
}
```

Wait until `status == "approved"` (or handle `denied` / `cancelled`). The **host** approves in the lobby UI (`approveExternalBotSeat`).

### 3. Wait for game start

Poll `lobby(id)` until `status == "in_game"` and `gameInstanceId` is set.

## WebSocket (bot mode)

Connect to the game instance:

```text
wss://<host>/game?id=<gameInstanceId>&player=<playerIdentity>&mode=bot&token=<connectToken>
```

- `player` — URL-encoded `player_identity` from the approved seat.
- `token` — the `connectToken` from the seat request (not the API key).

### Inbound frames (server → bot)

Each text message is JSON:

- **Initial and every tick:** full `PlayerState` object (game-specific JSON).
- **Game end:** `{"GameOver": ...}` (top-level `GameOver` key).

### Outbound frames (bot → server)

When `decide()` returns `Some(action)`, send the action as a **text** JSON frame (serialized `Action`).

### Bot settings

- Register with optional `settingsSchemaJson` and `settingsJson` on `registerExternalBot`.
- Pass `settingsJson` on `requestExternalBotSeat` (validated against schema server-side).
- After approval, effective settings are stored on the seat (`botSettingsJson` on lobby seat snapshot).
- Your runner should use the same settings object passed to `decide(settings, playerState)` in your bot logic.
- Global defaults can be updated with `updateBotSettings(botId, settingsJson)`.

See [schema-driven-settings.md](schema-driven-settings.md).

## End-to-end pseudo-code

```python
API_KEY = "gbk_..."
LOBBY_ID = "..."
CONTRACT_HASH = "..."  # from gameContract

# 1. Request seat
resp = gql("""
  mutation($id: ID!, $hash: String!) {
    requestExternalBotSeat(
      lobbyId: $id, category: "external", label: "MyBot", contractHash: $hash
    ) { requestId connectToken }
  }
""", variables={"id": LOBBY_ID, "hash": CONTRACT_HASH}, bearer=API_KEY)

request_id = resp["requestId"]
connect_token = resp["connectToken"]

# 2. Poll approval
while True:
    r = gql("query($id: ID!) { botRequest(requestId: $id) { status seatIndex } }",
            {"id": request_id}, bearer=API_KEY)
    if r["status"] == "approved":
        seat_index = r["seatIndex"]
        break
    if r["status"] in ("denied", "cancelled"):
        raise SystemExit("seat denied")

# 3. Wait for in_game + resolve player identity from lobby.seats[seat_index]

# 4. WebSocket loop
ws = connect(f"/game?id={game_id}&player={player}&mode=bot&token={connect_token}")
for frame in ws:
    if "GameOver" in frame:
        break
    state = parse_player_state(frame)
    action = decide(state)
    if action is not None:
        ws.send(json.dumps(action))
```

## Bot identity

Approved seats carry `bot_id`, `bot_display_name`, and avatar fields for match results. External bots use their registered `bots.id`. Dev-local bots use a transient id per run (`is_transient` in seat snapshots).

## Related

- Dev-local runner: `gamedev bot-run --lobby <uuid>` from a bot project.
- GraphQL URL: typically `/graphql` on the platform backend.
- Local dev: lobby on `:8080`, API/WS on `:8081`.
