//! Synthetic GraphQL responses for demo mode — makes the platform feel alive offline.

use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    static DEMO_LOBBY_GAME_TYPES: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
    static DEMO_LOBBY_OWNERS: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
}

fn now() -> i64 {
    (js_sys::Date::now() / 1000.0) as i64
}

fn ago_mins(m: i64) -> i64 {
    now() - m * 60
}

fn ago_hours(h: i64) -> i64 {
    now() - h * 3600
}

fn ago_days(d: i64) -> i64 {
    now() - d * 86_400
}

fn game_type_from_vars(variables: &Option<Value>) -> String {
    variables
        .as_ref()
        .and_then(|v| {
            v.get("t")
                .or_else(|| v.get("gameType"))
                .and_then(|x| x.as_str())
        })
        .unwrap_or("tic_tac_toe")
        .to_string()
}

fn game_type_entry(
    name: &str,
    display_name: &str,
    version: &str,
    min_players: u32,
    max_players: u32,
    description: &str,
) -> Value {
    use crate::stub::demo_images::cover_image_url;
    json!({
        "name": name,
        "displayName": display_name,
        "version": version,
        "minPlayers": min_players,
        "maxPlayers": max_players,
        "description": description,
        "configUiPath": "config.html",
        "aboutUiPath": "about.html",
        "configSchemaJson": null,
        "coverImageUrl": cover_image_url(name),
    })
}

fn game_types() -> Value {
    json!([
        game_type_entry(
            "tic_tac_toe",
            "Tic Tac Toe",
            "1.0.0",
            2,
            4,
            "Classic N×N tic-tac-toe with configurable board size and win length.",
        ),
        game_type_entry(
            "checkers",
            "Checkers",
            "0.2.1",
            2,
            2,
            "Traditional checkers with WASM-powered logic. Head-to-head in the lobby.",
        ),
        game_type_entry(
            "chess",
            "Chess",
            "0.4.0",
            2,
            2,
            "Standard chess with lobby timers, move validation, and optional blitz clocks.",
        ),
        game_type_entry(
            "connect_four",
            "Connect Four",
            "0.1.2",
            2,
            2,
            "Drop discs and connect four in a row — quick rounds perfect for party lobbies.",
        ),
        game_type_entry(
            "backgammon",
            "Backgammon",
            "0.3.0",
            2,
            2,
            "Race your checkers home with dice rolls. Doubling cube and match play in config.",
        ),
        game_type_entry(
            "go",
            "Go",
            "0.2.0",
            2,
            2,
            "Territory and capture on 9×9, 13×13, or full 19×19 boards with handicap stones.",
        ),
        game_type_entry(
            "reversi",
            "Reversi",
            "0.1.0",
            2,
            2,
            "Flip discs to control the board — classic Othello rules with fast lobby matchmaking.",
        ),
        game_type_entry(
            "catan",
            "Catan",
            "0.3.0",
            3,
            4,
            "Trade resources and build settlements — the classic 3–4 player euro strategy experience.",
        ),
        game_type_entry(
            "monopoly",
            "Monopoly",
            "0.2.0",
            2,
            8,
            "Buy, sell, and bankrupt your friends. Supports up to eight players with house rules in the lobby.",
        ),
        game_type_entry(
            "risk",
            "Risk",
            "0.1.5",
            2,
            6,
            "Conquer territories and eliminate rivals on a world map. Two to six players, mission or classic mode.",
        ),
        game_type_entry(
            "scrabble",
            "Scrabble",
            "0.1.1",
            2,
            4,
            "Spell high-scoring words on a shared board. Two to four players with built-in dictionary checks.",
        ),
        game_type_entry(
            "chinese_checkers",
            "Chinese Checkers",
            "0.2.0",
            2,
            6,
            "Hop marbles across the star board. Two to six players with optional team seating.",
        ),
        game_type_entry(
            "mahjong",
            "Mahjong",
            "0.1.0",
            4,
            4,
            "Four-player tile melds and scoring. Riichi or classical rules configured before the match starts.",
        ),
    ])
}

fn lobbies() -> Value {
    json!([
        { "id": "lob-a1f2", "gameType": "tic_tac_toe", "status": "waiting", "seatsFilled": 1, "seatsTotal": 4, "ownerDisplayName": "NovaPilot", "gameInstanceId": null, "createdAt": ago_mins(3) },
        { "id": "lob-b8c4", "gameType": "checkers", "status": "in_game", "seatsFilled": 2, "seatsTotal": 2, "ownerDisplayName": "CipherFox", "gameInstanceId": "game-chk-991", "createdAt": ago_mins(12) },
        { "id": "lob-c3d9", "gameType": "chess", "status": "waiting", "seatsFilled": 1, "seatsTotal": 2, "ownerDisplayName": "ByteRunner", "gameInstanceId": null, "createdAt": ago_mins(18) },
        { "id": "lob-d7e1", "gameType": "connect_four", "status": "full", "seatsFilled": 2, "seatsTotal": 2, "ownerDisplayName": "GridLock", "gameInstanceId": null, "createdAt": ago_mins(25) },
        { "id": "lob-e2f6", "gameType": "checkers", "status": "waiting", "seatsFilled": 1, "seatsTotal": 2, "ownerDisplayName": "PulseWave", "gameInstanceId": null, "createdAt": ago_mins(31) },
        { "id": "lob-f9a3", "gameType": "go", "status": "in_game", "seatsFilled": 2, "seatsTotal": 2, "ownerDisplayName": "ShadowArc", "gameInstanceId": "game-go-442", "createdAt": ago_mins(44) },
        { "id": "lob-g4b7", "gameType": "backgammon", "status": "waiting", "seatsFilled": 0, "seatsTotal": 2, "ownerDisplayName": "Guest", "gameInstanceId": null, "createdAt": ago_mins(52) },
        { "id": "lob-h1c8", "gameType": "reversi", "status": "waiting", "seatsFilled": 1, "seatsTotal": 2, "ownerDisplayName": "NovaPilot", "gameInstanceId": null, "createdAt": ago_hours(1) },
        { "id": "lob-i2d5", "gameType": "chess", "status": "in_game", "seatsFilled": 2, "seatsTotal": 2, "ownerDisplayName": "ArcLight", "gameInstanceId": "game-chs-118", "createdAt": ago_mins(8) },
        { "id": "lob-j6e9", "gameType": "connect_four", "status": "waiting", "seatsFilled": 1, "seatsTotal": 2, "ownerDisplayName": "CipherFox", "gameInstanceId": null, "createdAt": ago_mins(15) },
        { "id": "lob-k3f1", "gameType": "catan", "status": "waiting", "seatsFilled": 3, "seatsTotal": 4, "ownerDisplayName": "NovaPilot", "gameInstanceId": null, "createdAt": ago_mins(6) },
        { "id": "lob-l8g2", "gameType": "monopoly", "status": "in_game", "seatsFilled": 6, "seatsTotal": 8, "ownerDisplayName": "GridLock", "gameInstanceId": "game-mnp-204", "createdAt": ago_mins(20) },
        { "id": "lob-m1h7", "gameType": "risk", "status": "waiting", "seatsFilled": 4, "seatsTotal": 6, "ownerDisplayName": "ByteRunner", "gameInstanceId": null, "createdAt": ago_mins(27) },
        { "id": "lob-n4i0", "gameType": "scrabble", "status": "full", "seatsFilled": 4, "seatsTotal": 4, "ownerDisplayName": "PulseWave", "gameInstanceId": null, "createdAt": ago_mins(33) },
        { "id": "lob-o9j5", "gameType": "chinese_checkers", "status": "waiting", "seatsFilled": 4, "seatsTotal": 6, "ownerDisplayName": "ShadowArc", "gameInstanceId": null, "createdAt": ago_mins(39) },
        { "id": "lob-p2k8", "gameType": "mahjong", "status": "in_game", "seatsFilled": 4, "seatsTotal": 4, "ownerDisplayName": "ArcLight", "gameInstanceId": "game-mhj-881", "createdAt": ago_mins(48) },
        { "id": "lob-q6l3", "gameType": "catan", "status": "full", "seatsFilled": 4, "seatsTotal": 4, "ownerDisplayName": "CipherFox", "gameInstanceId": null, "createdAt": ago_hours(2) }
    ])
}

fn platform_stats() -> Value {
    json!({
        "activeLobbies": 17,
        "publishedGameTypes": 13,
        "finishedGames24h": 218,
        "activeSessions": 42,
        "status": "ok",
        "trends": [
            { "label": "Active lobbies", "value": "17", "deltaPct": "+12%", "up": true },
            { "label": "Published games", "value": "13", "deltaPct": "+2", "up": true },
            { "label": "Finished (24h)", "value": "218", "deltaPct": "+8%", "up": true }
        ],
        "proTip": "Demo mode — data is synthetic. Claim a seat and mark Ready before the host launches."
    })
}

fn activity_feed(limit: usize) -> Value {
    let all = json!([
        { "actor": "NovaPilot", "action": "won", "target": "Tic Tac Toe #TIC-88219", "timestamp": ago_mins(2) },
        { "actor": "CipherFox", "action": "joined lobby", "target": "Open Room #a3f2", "timestamp": ago_mins(5) },
        { "actor": "ByteRunner", "action": "published", "target": "Checkers v0.2.1", "timestamp": ago_mins(12) },
        { "actor": "GridLock", "action": "created lobby", "target": "Strategy Night", "timestamp": ago_mins(18) },
        { "actor": "PulseWave", "action": "claimed seat", "target": "Lobby #b91c", "timestamp": ago_mins(24) },
        { "actor": "ShadowArc", "action": "started match", "target": "Tic Tac Toe", "timestamp": ago_mins(31) },
        { "actor": "NovaPilot", "action": "reviewed", "target": "Checkers", "timestamp": ago_mins(38) },
        { "actor": "Guest", "action": "finished", "target": "Checkers #CHK-88077", "timestamp": ago_mins(45) },
        { "actor": "ByteRunner", "action": "deployed", "target": "tic_tac_toe v1.0.0", "timestamp": ago_hours(1) },
        { "actor": "CipherFox", "action": "won", "target": "Checkers #CHK-99102", "timestamp": ago_hours(2) },
        { "actor": "GridLock", "action": "commented on", "target": "Tic Tac Toe", "timestamp": ago_hours(3) },
        { "actor": "PulseWave", "action": "created lobby", "target": "Friday Night", "timestamp": ago_hours(4) },
        { "actor": "GridLock", "action": "started", "target": "Monopoly 6-player", "timestamp": ago_mins(19) },
        { "actor": "ArcLight", "action": "won", "target": "Mahjong #MHJ-88102", "timestamp": ago_mins(41) },
        { "actor": "NovaPilot", "action": "claimed seat", "target": "Catan lobby #k3f1", "timestamp": ago_mins(5) }
    ]);
    if let Some(arr) = all.as_array() {
        Value::Array(arr.iter().take(limit).cloned().collect())
    } else {
        all
    }
}

fn shot_json(id: &str, caption: &str, image_url: &str, gradient: &str) -> Value {
    json!({
        "id": id,
        "caption": caption,
        "imageUrl": image_url,
        "gradient": gradient,
    })
}

fn screenshots(game: &str) -> Value {
    use crate::stub::demo_images::screenshots_for_game;
    let gradients = [
        "from-primary-container/60 via-surface-container-low to-background",
        "from-secondary-container/50 via-primary-container/30 to-background",
        "from-tertiary-container/40 via-surface-container-high to-background",
        "from-primary/30 via-surface-container to-background",
        "from-tertiary/50 via-background to-surface-container-low",
    ];
    let list = screenshots_for_game(game);
    Value::Array(
        list.iter()
            .enumerate()
            .map(|(i, (id, caption, url))| {
                shot_json(id, caption, url, gradients.get(i).copied().unwrap_or(gradients[0]))
            })
            .collect(),
    )
}

fn patch_notes(game: &str) -> Value {
    match game {
        "checkers" => json!([
            { "version": "0.2.1", "date": "2026-04-08", "title": "Smoother captures & draw detection", "body": "Fixed edge-case forced jumps. Draw by repetition now triggers correctly after three identical positions.", "tags": ["Bugfix", "Balance"] },
            { "version": "0.2.0", "date": "2026-03-22", "title": "King movement polish", "body": "Kings can slide multiple squares. Added subtle move hints for new players.", "tags": ["Feature", "UX"] },
            { "version": "0.1.0", "date": "2026-02-10", "title": "Initial WASM release", "body": "Head-to-head checkers with lobby integration and result screen.", "tags": ["Release"] }
        ]),
        "chess" => json!([
            { "version": "0.4.0", "date": "2026-04-05", "title": "Blitz presets", "body": "Added 3+2, 5+0, and 10+0 clock presets in lobby config.", "tags": ["Feature"] },
            { "version": "0.3.0", "date": "2026-03-10", "title": "Draw offers", "body": "Players can offer/accept draws with server-side validation.", "tags": ["Feature"] }
        ]),
        "connect_four" => json!([
            { "version": "0.1.2", "date": "2026-04-02", "title": "Win animation", "body": "Highlight the winning four discs with a short celebration.", "tags": ["UX"] }
        ]),
        "backgammon" => json!([
            { "version": "0.3.0", "date": "2026-03-28", "title": "Doubling cube", "body": "Optional doubling cube with Crawford rule toggle.", "tags": ["Feature"] }
        ]),
        "go" => json!([
            { "version": "0.2.0", "date": "2026-03-20", "title": "Handicap stones", "body": "Lobby host can set 2–9 stone handicap for teaching games.", "tags": ["Feature"] }
        ]),
        "reversi" => json!([
            { "version": "0.1.0", "date": "2026-02-15", "title": "Initial release", "body": "Standard Othello rules with pass handling and disc flip animation.", "tags": ["Release"] }
        ]),
        "catan" => json!([
            { "version": "0.3.0", "date": "2026-04-06", "title": "Seafarers map toggle", "body": "Optional coastal expansion layout in lobby config.", "tags": ["Feature"] }
        ]),
        "monopoly" => json!([
            { "version": "0.2.0", "date": "2026-03-30", "title": "8-player support", "body": "Full octet lobbies with spectator slots.", "tags": ["Feature"] }
        ]),
        "risk" => json!([
            { "version": "0.1.5", "date": "2026-03-18", "title": "Mission cards", "body": "Secret objectives mode alongside classic elimination.", "tags": ["Feature"] }
        ]),
        "scrabble" => json!([
            { "version": "0.1.1", "date": "2026-03-12", "title": "Dictionary update", "body": "Expanded word list and blank tile UX.", "tags": ["UX"] }
        ]),
        "chinese_checkers" => json!([
            { "version": "0.2.0", "date": "2026-03-08", "title": "6-player star board", "body": "Team mode and diagonal opening setups.", "tags": ["Feature"] }
        ]),
        "mahjong" => json!([
            { "version": "0.1.0", "date": "2026-02-20", "title": "Riichi scoring", "body": "Four-seat tile walls with riichi or classical scoring.", "tags": ["Release"] }
        ]),
        _ => json!([
            { "version": "1.0.0", "date": "2026-04-01", "title": "Store page & aspect reviews", "body": "Steam-style game pages with spider-chart ratings, patch notes, and dual leaderboards.", "tags": ["Feature", "Platform"] },
            { "version": "0.9.2", "date": "2026-03-15", "title": "4-player support", "body": "Free-for-all mode on larger boards. Improved turn timer in lobby.", "tags": ["Feature"] },
            { "version": "0.9.0", "date": "2026-02-28", "title": "Configurable N×N boards", "body": "Board size and win length now configurable from the lobby config UI.", "tags": ["Feature"] },
            { "version": "0.8.0", "date": "2026-01-12", "title": "First public beta", "body": "Classic 3×3 mode with real-time lobby seats.", "tags": ["Release"] }
        ]),
    }
}

fn storefront(game: &str) -> Value {
    let (tagline, desc, tags, mins) = match game {
        "checkers" => (
            "Classic board combat — claim your seat",
            "Traditional checkers rebuilt for the IPEL lobby. WASM game logic, real-time seats, and post-match leaderboards.\n\nPerfect for quick 1v1 sessions or tournament nights with friends.",
            vec!["Board", "Turn-based", "1v1", "WASM"],
            14,
        ),
        "chess" => (
            "The immortal game — blitz or classical",
            "Full chess rules with lobby-configurable clocks. From five-minute blitz to untimed friendly matches.\n\nMove validation runs in WASM; results feed the dual leaderboards.",
            vec!["Classic", "1v1", "Ranked", "Timers"],
            28,
        ),
        "connect_four" => (
            "Drop four — win in a flash",
            "Vertical strategy that's easy to learn and hard to master. Perfect warm-up between longer board sessions.\n\nSpectator-friendly and great for streaming lobby nights.",
            vec!["Party", "Quick", "2P", "Family"],
            5,
        ),
        "backgammon" => (
            "Roll, race, and bear off",
            "One of the oldest board games, tuned for async-friendly lobby play. Optional doubling cube and match lengths.\n\nDice fairness verified server-side.",
            vec!["Dice", "Classic", "2P", "Luck+Skill"],
            18,
        ),
        "go" => (
            "Stones, territory, centuries of depth",
            "Play on 9×9 for quick teaching games or full 19×19 for serious matches. Handicap stones keep games balanced.\n\nScoring modes configurable per lobby.",
            vec!["Abstract", "Deep", "2P", "Territory"],
            35,
        ),
        "reversi" => (
            "Flip the board — control the corners",
            "Othello-style reversi with crisp disc animations and pass handling. Short games that reward positional play.\n\nIdeal for ranked ladder grinders.",
            vec!["Strategy", "Flip", "2P", "Quick"],
            10,
        ),
        "catan" => (
            "Trade, build, and race to 10 VP",
            "The lobby edition of Catan. Roll for resources, trade with seats at the table, and race to ten victory points.\n\nRobber, longest road, and largest army all supported.",
            vec!["Euro", "Trading", "3–4P", "Party"],
            65,
        ),
        "monopoly" => (
            "Pass Go, collect rent, repeat",
            "Classic property trading for two to eight players. Auction rules, free parking jackpot, and speed die are lobby toggles.\n\nExpect long sessions and table talk.",
            vec!["Party", "Economy", "2–8P", "Classic"],
            85,
        ),
        "risk" => (
            "Conquer the map — alliances optional",
            "World domination with two to six commanders. Reinforce, attack, and fortify across territories.\n\nMission cards or last-player-standing modes available.",
            vec!["Strategy", "War", "2–6P", "Long"],
            120,
        ),
        "scrabble" => (
            "Words win points — challenge accepted",
            "Shared board word building for two to four players. Premium squares, bingo bonuses, and dictionary validation.\n\nGreat for mixed-skill family lobbies.",
            vec!["Words", "Family", "2–4P", "Turn-based"],
            45,
        ),
        "chinese_checkers" => (
            "Hop your marbles home first",
            "Star-board marble racing for two to six players. Chain hops, blocking, and team variants.\n\nFast enough for between-match filler at game nights.",
            vec!["Party", "Hops", "2–6P", "Light"],
            22,
        ),
        "mahjong" => (
            "Four seats, one wall of tiles",
            "Complete four-player mahjong with riichi or classical scoring. Pungs, chows, and concealed hands.\n\nSeat order and prevailing wind rotate each round.",
            vec!["Tiles", "4P", "Classic", "Scoring"],
            55,
        ),
        _ => (
            "Quick strategy — from 3×3 to custom grids",
            "The definitive lobby tic-tac-toe experience. Scale from classic 3×3 duels to custom N×N boards with configurable win lengths.\n\nFour-player free-for-all supported. Configure everything before you launch.",
            vec!["Strategy", "Classic", "Party", "Configurable"],
            8,
        ),
    };
    let (aspects, review_count) = aspect_ratings(game);
    json!({
        "gameName": game,
        "shortTagline": tagline,
        "longDescription": desc,
        "screenshots": screenshots(game),
        "patchNotes": patch_notes(game),
        "tags": tags,
        "avgSessionMins": mins,
        "aspectRatings": aspects,
        "reviewCount": review_count,
        "canEdit": true,
        "updatedAt": ago_hours(2)
    })
}

/// Per-game spider-chart averages — shapes reflect each game's strengths.
fn aspect_ratings(game: &str) -> (Value, i32) {
    match game {
        "chess" => (
            json!({ "gameplay": 4.9, "balance": 4.8, "visuals": 4.2, "social": 3.4, "depth": 5.0 }),
            41,
        ),
        "go" => (
            json!({ "gameplay": 4.7, "balance": 4.9, "visuals": 4.4, "social": 3.2, "depth": 5.0 }),
            28,
        ),
        "checkers" => (
            json!({ "gameplay": 4.3, "balance": 4.4, "visuals": 3.8, "social": 4.1, "depth": 3.7 }),
            24,
        ),
        "backgammon" => (
            json!({ "gameplay": 4.1, "balance": 3.4, "visuals": 4.0, "social": 4.3, "depth": 3.9 }),
            19,
        ),
        "connect_four" => (
            json!({ "gameplay": 4.0, "balance": 3.7, "visuals": 4.6, "social": 4.8, "depth": 2.4 }),
            31,
        ),
        "reversi" => (
            json!({ "gameplay": 4.2, "balance": 4.1, "visuals": 3.9, "social": 4.0, "depth": 3.8 }),
            16,
        ),
        "tic_tac_toe" => (
            json!({ "gameplay": 4.2, "balance": 4.0, "visuals": 3.6, "social": 4.7, "depth": 3.1 }),
            36,
        ),
        "catan" => (
            json!({ "gameplay": 4.6, "balance": 4.3, "visuals": 4.5, "social": 4.9, "depth": 4.5 }),
            34,
        ),
        "monopoly" => (
            json!({ "gameplay": 3.8, "balance": 3.2, "visuals": 4.4, "social": 4.9, "depth": 3.0 }),
            42,
        ),
        "risk" => (
            json!({ "gameplay": 4.1, "balance": 3.8, "visuals": 4.0, "social": 4.5, "depth": 4.3 }),
            27,
        ),
        "scrabble" => (
            json!({ "gameplay": 4.3, "balance": 4.2, "visuals": 3.9, "social": 4.4, "depth": 4.0 }),
            22,
        ),
        "chinese_checkers" => (
            json!({ "gameplay": 4.1, "balance": 4.0, "visuals": 4.5, "social": 4.7, "depth": 3.4 }),
            18,
        ),
        "mahjong" => (
            json!({ "gameplay": 4.5, "balance": 4.4, "visuals": 4.3, "social": 4.8, "depth": 4.6 }),
            29,
        ),
        _ => (
            json!({ "gameplay": 4.0, "balance": 4.0, "visuals": 3.8, "social": 4.0, "depth": 3.5 }),
            12,
        ),
    }
}

fn reviews(game: &str) -> Value {
    match game {
        "chess" => json!([
            { "id": "rev-ch-1", "displayName": "ArcLight", "body": "Blitz presets are perfect. The spider chart nails it — depth and balance are sky-high.", "aspects": { "gameplay": 5.0, "balance": 5.0, "visuals": 4.0, "social": 3.0, "depth": 5.0 }, "helpfulVotes": 22, "createdAt": ago_mins(55) },
            { "id": "rev-ch-2", "displayName": "CipherFox", "body": "Draw offers work flawlessly. Classical lobbies feel serious — low social score fits.", "aspects": { "gameplay": 4.8, "balance": 4.9, "visuals": 4.2, "social": 3.5, "depth": 5.0 }, "helpfulVotes": 14, "createdAt": ago_hours(4) },
            { "id": "rev-ch-3", "displayName": "NovaPilot", "body": "Best browser chess I've tried in a lobby context.", "aspects": { "gameplay": 5.0, "balance": 4.7, "visuals": 4.0, "social": 3.5, "depth": 4.8 }, "helpfulVotes": 9, "createdAt": ago_hours(8) }
        ]),
        "go" => json!([
            { "id": "rev-go-1", "displayName": "ByteRunner", "body": "Handicap stones make teaching games actually fun. Depth axis maxed out, deservedly.", "aspects": { "gameplay": 4.8, "balance": 5.0, "visuals": 4.5, "social": 3.0, "depth": 5.0 }, "helpfulVotes": 11, "createdAt": ago_mins(70) },
            { "id": "rev-go-2", "displayName": "ShadowArc", "body": "19×19 scoring is clean. Quiet game — social rating lower than party titles.", "aspects": { "gameplay": 4.6, "balance": 4.8, "visuals": 4.3, "social": 3.2, "depth": 5.0 }, "helpfulVotes": 7, "createdAt": ago_hours(6) }
        ]),
        "checkers" => json!([
            { "id": "rev-ck-1", "displayName": "PulseWave", "body": "Forced jumps feel fair now. Great head-to-head filler between longer games.", "aspects": { "gameplay": 4.4, "balance": 4.5, "visuals": 3.8, "social": 4.0, "depth": 3.5 }, "helpfulVotes": 16, "createdAt": ago_mins(45) },
            { "id": "rev-ck-2", "displayName": "GridLock", "body": "King slides are satisfying. Draw detection patch was needed.", "aspects": { "gameplay": 4.2, "balance": 4.3, "visuals": 3.7, "social": 4.2, "depth": 3.8 }, "helpfulVotes": 10, "createdAt": ago_hours(5) }
        ]),
        "backgammon" => json!([
            { "id": "rev-bg-1", "displayName": "Guest", "body": "Dice variance shows in the balance score — accurate! Still tons of skill.", "aspects": { "gameplay": 4.2, "balance": 3.2, "visuals": 4.0, "social": 4.5, "depth": 4.0 }, "helpfulVotes": 8, "createdAt": ago_mins(90) },
            { "id": "rev-bg-2", "displayName": "NovaPilot", "body": "Doubling cube toggle is a nice lobby option.", "aspects": { "gameplay": 4.0, "balance": 3.5, "visuals": 4.1, "social": 4.0, "depth": 3.8 }, "helpfulVotes": 5, "createdAt": ago_hours(10) }
        ]),
        "connect_four" => json!([
            { "id": "rev-c4-1", "displayName": "CipherFox", "body": "Social and visuals spike on the chart — exactly how this game feels at parties.", "aspects": { "gameplay": 4.0, "balance": 3.5, "visuals": 5.0, "social": 5.0, "depth": 2.0 }, "helpfulVotes": 19, "createdAt": ago_mins(30) },
            { "id": "rev-c4-2", "displayName": "ByteRunner", "body": "Two-minute rounds between chess matches. Win animation is cute.", "aspects": { "gameplay": 4.2, "balance": 3.8, "visuals": 4.5, "social": 4.7, "depth": 2.5 }, "helpfulVotes": 12, "createdAt": ago_hours(2) }
        ]),
        "reversi" => json!([
            { "id": "rev-re-1", "displayName": "GridLock", "body": "Corner fights are addictive. Balanced chart — not too deep, not too shallow.", "aspects": { "gameplay": 4.3, "balance": 4.2, "visuals": 3.8, "social": 4.0, "depth": 3.9 }, "helpfulVotes": 6, "createdAt": ago_hours(7) }
        ]),
        "catan" => json!([
            { "id": "rev-ca-1", "displayName": "NovaPilot", "body": "Trading in chat is half the fun — social axis is spot on.", "aspects": { "gameplay": 4.7, "balance": 4.3, "visuals": 4.5, "social": 5.0, "depth": 4.5 }, "helpfulVotes": 15, "createdAt": ago_mins(50) },
            { "id": "rev-ca-2", "displayName": "CipherFox", "body": "4-seat lobbies fill fast on weekends.", "aspects": { "gameplay": 4.5, "balance": 4.2, "visuals": 4.4, "social": 4.8, "depth": 4.4 }, "helpfulVotes": 8, "createdAt": ago_hours(5) }
        ]),
        "monopoly" => json!([
            { "id": "rev-mn-1", "displayName": "GridLock", "body": "8-player chaos is why social is maxed. Balance suffers and we know it.", "aspects": { "gameplay": 3.5, "balance": 2.8, "visuals": 4.5, "social": 5.0, "depth": 2.5 }, "helpfulVotes": 21, "createdAt": ago_mins(35) }
        ]),
        "risk" => json!([
            { "id": "rev-rk-1", "displayName": "ByteRunner", "body": "6-player alliances and betrayals — depth and social both high.", "aspects": { "gameplay": 4.2, "balance": 3.5, "visuals": 4.0, "social": 4.6, "depth": 4.5 }, "helpfulVotes": 11, "createdAt": ago_hours(9) }
        ]),
        "scrabble" => json!([
            { "id": "rev-sc-1", "displayName": "PulseWave", "body": "Dictionary challenges work great in the lobby.", "aspects": { "gameplay": 4.4, "balance": 4.3, "visuals": 3.8, "social": 4.3, "depth": 4.1 }, "helpfulVotes": 7, "createdAt": ago_hours(4) }
        ]),
        "chinese_checkers" => json!([
            { "id": "rev-cc-1", "displayName": "ShadowArc", "body": "6-player star board is perfect party filler.", "aspects": { "gameplay": 4.2, "balance": 4.0, "visuals": 4.6, "social": 4.8, "depth": 3.2 }, "helpfulVotes": 9, "createdAt": ago_mins(65) }
        ]),
        "mahjong" => json!([
            { "id": "rev-mj-1", "displayName": "ArcLight", "body": "Riichi scoring tutorial in the lobby helped our table a lot.", "aspects": { "gameplay": 4.6, "balance": 4.5, "visuals": 4.2, "social": 4.9, "depth": 4.7 }, "helpfulVotes": 13, "createdAt": ago_hours(6) }
        ]),
        _ => json!([
            { "id": "rev-1", "displayName": "NovaPilot", "body": "Surprisingly fun at 5×5 win-4. Social axis is high — four-player lobbies are chaos.", "aspects": { "gameplay": 4.5, "balance": 4.0, "visuals": 3.5, "social": 5.0, "depth": 3.0 }, "helpfulVotes": 18, "createdAt": ago_mins(40) },
            { "id": "rev-2", "displayName": "CipherFox", "body": "Quick rounds, easy config. Depth stays modest and that's fine.", "aspects": { "gameplay": 4.0, "balance": 4.0, "visuals": 3.6, "social": 4.5, "depth": 2.8 }, "helpfulVotes": 12, "createdAt": ago_hours(3) },
            { "id": "rev-3", "displayName": "ByteRunner", "body": "Great for teaching kids in the lobby.", "aspects": { "gameplay": 4.2, "balance": 4.1, "visuals": 3.5, "social": 4.8, "depth": 3.2 }, "helpfulVotes": 9, "createdAt": ago_hours(6) },
            { "id": "rev-4", "displayName": "GridLock", "body": "Four-player free-for-all is the real mode.", "aspects": { "gameplay": 4.5, "balance": 3.5, "visuals": 3.5, "social": 5.0, "depth": 3.5 }, "helpfulVotes": 7, "createdAt": ago_hours(12) }
        ]),
    }
}

fn comments() -> Value {
    json!([
        { "id": "c1", "displayName": "CipherFox", "body": "Anyone up for a 5×5 win-4 lobby tonight?", "createdAt": ago_mins(8) },
        { "id": "c2", "displayName": "NovaPilot", "body": "I'm in — creating a room in 10 min.", "createdAt": ago_mins(7) },
        { "id": "c3", "displayName": "ByteRunner", "body": "The new patch fixed the draw bug, nice.", "createdAt": ago_mins(22) },
        { "id": "c4", "displayName": "GridLock", "body": "Tips for 4-player? I keep getting sandwiched.", "createdAt": ago_mins(35) },
        { "id": "c5", "displayName": "PulseWave", "body": "Control the center on larger boards.", "createdAt": ago_mins(33) },
        { "id": "c6", "displayName": "ShadowArc", "body": "Dev — any plans for ranked seasons?", "createdAt": ago_hours(2) },
        { "id": "c7", "displayName": "Guest", "body": "First time here, this platform feels great.", "createdAt": ago_hours(3) },
        { "id": "c8", "displayName": "NovaPilot", "body": "GG to everyone in the last tournament thread.", "createdAt": ago_hours(5) },
        { "id": "c9", "displayName": "CipherFox", "body": "Checkers lobby #b8c4 is live if anyone wants 1v1.", "createdAt": ago_hours(6) },
        { "id": "c10", "displayName": "ByteRunner", "body": "Spider chart ratings are way more useful than stars.", "createdAt": ago_hours(8) }
    ])
}

fn finished_sessions(game: &str) -> Value {
    let prefix = match game {
        "checkers" => "CHK",
        "chess" => "CHS",
        "connect_four" => "C4",
        "backgammon" => "BGM",
        "go" => "GO",
        "reversi" => "REV",
        "catan" => "CAT",
        "monopoly" => "MNP",
        "risk" => "RSK",
        "scrabble" => "SCB",
        "chinese_checkers" => "CHK6",
        "mahjong" => "MHJ",
        _ => "TIC",
    };
    json!([
        { "gameId": format!("{prefix}-88219"), "gameType": game, "finishedAt": ago_mins(15), "winnerDisplayName": "NovaPilot", "participantCount": 2, "durationSecs": 540 },
        { "gameId": format!("{prefix}-88104"), "gameType": game, "finishedAt": ago_mins(28), "winnerDisplayName": null, "participantCount": 2, "durationSecs": 320 },
        { "gameId": format!("{prefix}-88077"), "gameType": game, "finishedAt": ago_mins(42), "winnerDisplayName": "CipherFox", "participantCount": 2, "durationSecs": 720 },
        { "gameId": format!("{prefix}-88012"), "gameType": game, "finishedAt": ago_hours(1), "winnerDisplayName": "ByteRunner", "participantCount": 4, "durationSecs": 480 },
        { "gameId": format!("{prefix}-87955"), "gameType": game, "finishedAt": ago_hours(2), "winnerDisplayName": "GridLock", "participantCount": 2, "durationSecs": 390 },
        { "gameId": format!("{prefix}-87890"), "gameType": game, "finishedAt": ago_hours(3), "winnerDisplayName": "PulseWave", "participantCount": 2, "durationSecs": 610 },
        { "gameId": format!("{prefix}-87801"), "gameType": game, "finishedAt": ago_hours(5), "winnerDisplayName": "ShadowArc", "participantCount": 3, "durationSecs": 840 },
        { "gameId": format!("{prefix}-87744"), "gameType": game, "finishedAt": ago_hours(8), "winnerDisplayName": "NovaPilot", "participantCount": 2, "durationSecs": 295 }
    ])
}

fn points_leaderboard() -> Value {
    json!([
        { "rank": 1, "displayName": "NovaPilot", "totalScore": 2840, "wins": 42, "winRatePct": 78 },
        { "rank": 2, "displayName": "CipherFox", "totalScore": 2510, "wins": 38, "winRatePct": 71 },
        { "rank": 3, "displayName": "ByteRunner", "totalScore": 2190, "wins": 34, "winRatePct": 68 },
        { "rank": 4, "displayName": "GridLock", "totalScore": 1920, "wins": 29, "winRatePct": 62 },
        { "rank": 5, "displayName": "PulseWave", "totalScore": 1650, "wins": 24, "winRatePct": 58 },
        { "rank": 6, "displayName": "ShadowArc", "totalScore": 1400, "wins": 19, "winRatePct": 51 },
        { "rank": 7, "displayName": "Guest", "totalScore": 980, "wins": 12, "winRatePct": 44 },
        { "rank": 8, "displayName": "ArcLight", "totalScore": 720, "wins": 8, "winRatePct": 40 }
    ])
}

fn playtime_leaderboard() -> Value {
    json!([
        { "rank": 1, "displayName": "NovaPilot", "totalMins": 1240, "sessions": 89 },
        { "rank": 2, "displayName": "CipherFox", "totalMins": 980, "sessions": 72 },
        { "rank": 3, "displayName": "ByteRunner", "totalMins": 760, "sessions": 54 },
        { "rank": 4, "displayName": "GridLock", "totalMins": 620, "sessions": 41 },
        { "rank": 5, "displayName": "PulseWave", "totalMins": 480, "sessions": 35 },
        { "rank": 6, "displayName": "ShadowArc", "totalMins": 390, "sessions": 28 },
        { "rank": 7, "displayName": "Guest", "totalMins": 210, "sessions": 18 },
        { "rank": 8, "displayName": "ArcLight", "totalMins": 145, "sessions": 11 }
    ])
}

fn my_profile() -> Value {
    json!({
        "displayName": "NovaPilot",
        "createdAt": ago_days(120),
        "matchesPlayed": 142,
        "gamesPublished": 2,
        "wins": 89,
        "repScore": 2840
    })
}

fn my_badges() -> Value {
    json!([
        { "id": "veteran", "label": "Veteran", "tier": "Gold", "locked": false, "earnedAt": ago_days(30) },
        { "id": "publisher", "label": "Publisher", "tier": "Silver", "locked": false, "earnedAt": ago_days(14) },
        { "id": "streak", "label": "Win Streak", "tier": "Bronze", "locked": false, "earnedAt": ago_days(7) },
        { "id": "elite", "label": "Elite Operator", "tier": "Elite", "locked": false, "earnedAt": ago_days(2) },
        { "id": "architect", "label": "Architect", "tier": "Elite", "locked": true, "earnedAt": null },
        { "id": "mentor", "label": "Mentor", "tier": "Gold", "locked": true, "earnedAt": null }
    ])
}

fn my_notifications() -> Value {
    json!([
        { "id": "n1", "title": "Lobby started", "body": "Your tic_tac_toe match is now in progress.", "kind": "lobby", "unread": true, "createdAt": ago_mins(2) },
        { "id": "n2", "title": "New game published", "body": "Tic Tac Toe v1.0.0 is live on this server.", "kind": "publish", "unread": true, "createdAt": ago_hours(1) },
        { "id": "n3", "title": "Developer access", "body": "Upload console is available for your account.", "kind": "system", "unread": false, "createdAt": ago_days(1) }
    ])
}

fn my_tokens() -> Value {
    json!([
        { "id": "tok-demo-1", "label": "CI Pipeline", "maskedKey": "ipel_••••••••4f2a", "createdAt": ago_days(14), "expiresAt": ago_days(-90) },
        { "id": "tok-demo-2", "label": "Local Dev", "maskedKey": "ipel_••••••••9b1c", "createdAt": ago_days(45), "expiresAt": ago_days(-60) }
    ])
}

fn my_drafts() -> Value {
    json!([
        { "id": "draft-1", "gameName": "tic_tac_toe", "displayName": "Tic Tac Toe", "version": "1.1.0", "status": "validated", "manifestJson": r#"{"description":"Next minor with UX polish"}"#, "createdAt": ago_days(3), "publishedAt": null },
        { "id": "draft-2", "gameName": "chess", "displayName": "Chess", "version": "0.5.0", "status": "draft", "manifestJson": r#"{"description":"Fischer random experiment"}"#, "createdAt": ago_days(7), "publishedAt": null },
        { "id": "draft-3", "gameName": "tic_tac_toe", "displayName": "Tic Tac Toe", "version": "1.0.0", "status": "published", "manifestJson": r#"{"description":"Live on platform"}"#, "createdAt": ago_days(90), "publishedAt": ago_days(30) },
        { "id": "draft-4", "gameName": "go", "displayName": "Go", "version": "0.2.1", "status": "validated", "manifestJson": r#"{"description":"Byo-yomi timer polish"}"#, "createdAt": ago_days(5), "publishedAt": null }
    ])
}

fn deployments() -> Value {
    json!([
        { "id": "dep-001", "gameName": "tic_tac_toe", "displayName": "Tic Tac Toe", "version": "1.0.0", "status": "live", "deployedAt": ago_hours(2) },
        { "id": "dep-002", "gameName": "checkers", "displayName": "Checkers", "version": "0.2.1", "status": "live", "deployedAt": ago_days(1) },
        { "id": "dep-003", "gameName": "chess", "displayName": "Chess", "version": "0.4.0", "status": "live", "deployedAt": ago_days(2) },
        { "id": "dep-004", "gameName": "connect_four", "displayName": "Connect Four", "version": "0.1.2", "status": "live", "deployedAt": ago_days(3) },
        { "id": "dep-005", "gameName": "backgammon", "displayName": "Backgammon", "version": "0.3.0", "status": "live", "deployedAt": ago_days(5) },
        { "id": "dep-006", "gameName": "go", "displayName": "Go", "version": "0.2.0", "status": "live", "deployedAt": ago_days(6) },
        { "id": "dep-007", "gameName": "reversi", "displayName": "Reversi", "version": "0.1.0", "status": "live", "deployedAt": ago_days(8) },
        { "id": "dep-008", "gameName": "catan", "displayName": "Catan", "version": "0.3.0", "status": "live", "deployedAt": ago_days(4) },
        { "id": "dep-009", "gameName": "monopoly", "displayName": "Monopoly", "version": "0.2.0", "status": "live", "deployedAt": ago_days(5) },
        { "id": "dep-010", "gameName": "risk", "displayName": "Risk", "version": "0.1.5", "status": "live", "deployedAt": ago_days(7) },
        { "id": "dep-011", "gameName": "scrabble", "displayName": "Scrabble", "version": "0.1.1", "status": "live", "deployedAt": ago_days(9) },
        { "id": "dep-012", "gameName": "chinese_checkers", "displayName": "Chinese Checkers", "version": "0.2.0", "status": "live", "deployedAt": ago_days(10) },
        { "id": "dep-013", "gameName": "mahjong", "displayName": "Mahjong", "version": "0.1.0", "status": "live", "deployedAt": ago_days(11) }
    ])
}

fn lobby_id_from_vars(variables: &Option<Value>) -> String {
    variables
        .as_ref()
        .and_then(|v| v.get("id").and_then(|x| x.as_str()))
        .unwrap_or("lob-a1f2")
        .to_string()
}

fn demo_owner_for_lobby(id: &str, seats: &Value) -> (String, String) {
    let owner_id = DEMO_LOBBY_OWNERS
        .with(|m| m.borrow().get(id).cloned())
        .unwrap_or_else(|| "u-nova".into());
    let owner_name = seats
        .as_array()
        .and_then(|arr| {
            arr.iter().find_map(|s| {
                let uid = s.get("claimedByUserId").and_then(|v| v.as_str())?;
                if uid == owner_id {
                    s.get("claimedDisplayName")
                        .and_then(|v| v.as_str())
                        .map(str::to_string)
                } else {
                    None
                }
            })
        })
        .unwrap_or_else(|| "NovaPilot".into());
    (owner_id, owner_name)
}

fn lobby_detail(id: &str) -> Value {
    let game = DEMO_LOBBY_GAME_TYPES.with(|m| m.borrow().get(id).cloned()).unwrap_or_else(|| {
        if id == "demo-lobby-new" {
            String::new()
        } else if id.contains("b8") || id.contains("e2") {
            "checkers".into()
        } else {
            "tic_tac_toe".into()
        }
    });
    let seats = if game.is_empty() {
        json!([])
    } else if game == "checkers" {
        json!([
            { "seatIndex": 0, "playerIdentity": "p1", "claimedByUserId": "u-nova", "claimedDisplayName": "NovaPilot", "ready": true },
            { "seatIndex": 1, "playerIdentity": "p2", "claimedByUserId": "u-cipher", "claimedDisplayName": "CipherFox", "ready": true }
        ])
    } else {
        json!([
            { "seatIndex": 0, "playerIdentity": "p1", "claimedByUserId": "u-nova", "claimedDisplayName": "NovaPilot", "ready": true },
            { "seatIndex": 1, "playerIdentity": "p2", "claimedByUserId": null, "claimedDisplayName": null, "ready": false },
            { "seatIndex": 2, "playerIdentity": "p3", "claimedByUserId": "u-byte", "claimedDisplayName": "ByteRunner", "ready": false },
            { "seatIndex": 3, "playerIdentity": "p4", "claimedByUserId": null, "claimedDisplayName": null, "ready": false }
        ])
    };
    let (owner_id, owner_name) = demo_owner_for_lobby(id, &seats);
    json!({
        "id": id,
        "ownerUserId": owner_id,
        "ownerDisplayName": owner_name,
        "gameType": game,
        "configJson": if game.is_empty() { Value::Null } else { json!(r#"{"boardSize":5,"winLength":4}"#) },
        "status": "waiting",
        "gameInstanceId": null,
        "createdAt": ago_mins(10),
        "updatedAt": ago_mins(1),
        "seats": seats,
        "messages": [
            { "id": "m1", "userId": "u-nova", "displayName": "NovaPilot", "body": "Welcome! Config is 5×5 win-4.", "createdAt": ago_mins(9) },
            { "id": "m2", "userId": "u-byte", "displayName": "ByteRunner", "body": "Ready when you are.", "createdAt": ago_mins(5) },
            { "id": "m3", "userId": "u-cipher", "displayName": "CipherFox", "body": "Grabbing seat 2 if someone leaves.", "createdAt": ago_mins(2) }
        ]
    })
}

fn normalize_query(q: &str) -> String {
    q.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub async fn demo_graphql<T: DeserializeOwned>(
    query: &str,
    variables: Option<Value>,
) -> Result<T, String> {
    let q = normalize_query(query);
    let gt = game_type_from_vars(&variables);

    let data: Value = if q.contains("mutation") {
        if q.contains("registerUser") {
            json!({ "registerUser": { "id": "demo-user-nova" } })
        } else if q.contains("signUp") {
            json!({ "signUp": { "id": "demo-user-nova" } })
        } else if q.contains("loginWithPassword") {
            json!({ "loginWithPassword": { "id": "demo-user-nova" } })
        } else if q.contains("createLobby") {
            let id = "demo-lobby-new";
            let gt = variables
                .as_ref()
                .and_then(|v| {
                    v.get("gt")
                        .or_else(|| v.get("gameType"))
                        .and_then(|x| x.as_str())
                })
                .unwrap_or("");
            if !gt.is_empty() {
                DEMO_LOBBY_GAME_TYPES.with(|m| m.borrow_mut().insert(id.to_string(), gt.to_string()));
            }
            json!({ "createLobby": { "id": id } })
        } else if q.contains("setLobbyGameType") {
            let id = lobby_id_from_vars(&variables);
            let gt = game_type_from_vars(&variables);
            DEMO_LOBBY_GAME_TYPES.with(|m| m.borrow_mut().insert(id.clone(), gt));
            json!({ "setLobbyGameType": lobby_detail(&id) })
        } else if q.contains("transferLobbyOwnership") {
            let id = lobby_id_from_vars(&variables);
            let new_owner = variables
                .as_ref()
                .and_then(|v| v.get("u").and_then(|x| x.as_str()))
                .unwrap_or("u-byte")
                .to_string();
            DEMO_LOBBY_OWNERS.with(|m| m.borrow_mut().insert(id.clone(), new_owner));
            json!({ "transferLobbyOwnership": lobby_detail(&id) })
        } else if q.contains("submitGameReview") || q.contains("submitGameComment") || q.contains("updateGameStorefront") {
            if q.contains("updateGameStorefront") {
                json!({ "updateGameStorefront": true })
            } else if q.contains("submitGameReview") {
                json!({ "submitGameReview": { "id": "rev-new" } })
            } else {
                json!({ "submitGameComment": { "id": "c-new" } })
            }
        } else if q.contains("revokePublishToken") {
            json!({ "revokePublishToken": true })
        } else if q.contains("createPublishToken") {
            json!({ "createPublishToken": { "token": "ipel_demo_key_do_not_share", "expiresAt": ago_days(-90) } })
        } else if q.contains("uploadGameZip") {
            json!({ "uploadGameZip": { "report": { "ok": true, "errors": 0, "warnings": 1, "infos": 2, "requiredIndexHtml": true, "requiredConfigHtml": true, "requiredResultHtml": true, "requiredAboutHtml": true, "diagnostics": [
                { "severity": "info", "code": "DEMO", "message": "Demo mode — upload simulated", "path": null, "hint": null }
            ] } } })
        } else if q.contains("publishGameDraft") {
            json!({ "publishGameDraft": { "id": "draft-1" } })
        } else if q.contains("unpublishGameDraft") {
            json!({ "unpublishGameDraft": { "id": "draft-3", "status": "validated" } })
        } else if q.contains("discardGameDraft") {
            json!({ "discardGameDraft": true })
        } else if q.contains("updateGameDraft") {
            json!({ "updateGameDraft": { "id": "draft-1" } })
        } else {
            json!({ "ok": true })
        }
    } else if q.contains("gameStorefront") {
        json!({ "gameStorefront": storefront(&gt) })
    } else if q.contains("gameReviews") {
        json!({ "gameReviews": reviews(&gt) })
    } else if q.contains("gameComments") {
        json!({ "gameComments": comments() })
    } else if q.contains("finishedGamesByType") {
        json!({ "finishedGamesByType": finished_sessions(&gt) })
    } else if q.contains("gamePlayTimeLeaderboard") {
        json!({ "gamePlayTimeLeaderboard": playtime_leaderboard() })
    } else if q.contains("gameLeaderboard") {
        json!({ "gameLeaderboard": points_leaderboard() })
    } else if q.contains("activityFeed") {
        let limit = 12usize;
        json!({ "activityFeed": activity_feed(limit) })
    } else if q.contains("platformStats") {
        json!({ "platformStats": platform_stats() })
    } else if q.contains("myProfile") {
        json!({ "myProfile": my_profile() })
    } else if q.contains("myBadges") {
        json!({ "myBadges": my_badges() })
    } else if q.contains("myNotifications") {
        json!({ "myNotifications": my_notifications() })
    } else if q.contains("unreadNotificationCount") {
        json!({ "unreadNotificationCount": 2 })
    } else if q.contains("markAllNotificationsRead") {
        json!({ "markAllNotificationsRead": 2 })
    } else if q.contains("markNotificationRead") {
        json!({ "markNotificationRead": true })
    } else if q.contains("myPublishTokens") {
        json!({ "myPublishTokens": my_tokens() })
    } else if q.contains("isDeveloper") {
        json!({ "isDeveloper": true })
    } else if q.contains("myGameDrafts") {
        json!({ "myGameDrafts": my_drafts() })
    } else if q.contains("publishedDeployments") {
        json!({ "publishedDeployments": deployments() })
    } else if q.contains("lobby(") || q.contains("lobby ") {
        let id = variables
            .as_ref()
            .and_then(|v| v.get("id").and_then(|x| x.as_str()))
            .unwrap_or("lob-a1f2");
        json!({ "lobby": lobby_detail(id) })
    } else if q.contains("finishedGame") {
        json!({
            "finishedGame": {
                "gameId": "demo-result-1",
                "gameType": gt,
                "lobbyId": "lob-a1f2",
                "finishedAt": ago_mins(20),
                "resultJson": r#"{"winner":"NovaPilot"}"#,
                "playerScoresJson": r#"[{"player":"NovaPilot","score":10},{"player":"CipherFox","score":6}]"#,
                "seatsSnapshotJson": "[]",
                "resultUiPath": null
            }
        })
    } else if q.contains("user(") {
        let id = variables
            .as_ref()
            .and_then(|v| v.get("id").and_then(|x| x.as_str()))
            .unwrap_or("demo-user-nova");
        json!({ "user": { "id": id } })
    } else if q.contains("gameTypes") && q.contains("lobbies") {
        json!({ "gameTypes": game_types(), "lobbies": lobbies() })
    } else if q.contains("gameTypes") {
        json!({ "gameTypes": game_types() })
    } else if q.contains("lobbies") {
        json!({ "lobbies": lobbies() })
    } else if q.contains("gameInstances") {
        json!({ "gameInstances": [
            { "gameId": "game-tic-442", "gameType": "tic_tac_toe", "playerIdentities": ["p1","p2","p3"], "connectedPlayers": 3 },
            { "gameId": "game-chk-991", "gameType": "checkers", "playerIdentities": ["p1","p2"], "connectedPlayers": 2 }
        ]})
    } else {
        json!({})
    };

    serde_json::from_value(data).map_err(|e| format!("demo data parse: {e}"))
}
