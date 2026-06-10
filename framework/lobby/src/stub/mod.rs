//! Client-side stub data for UI features not yet backed by the GraphQL API.

pub mod demo_api;
pub mod demo_images;
pub mod demo_mode;

#[derive(Clone, Debug)]
pub struct GameMedia {
    pub accent_gradient: &'static str,
    pub icon_emoji: &'static str,
}

#[derive(Clone, Debug)]
pub struct GameStubMeta {
    pub tags: &'static [&'static str],
    pub active_players: u32,
    pub ping_ms: u32,
    pub featured: bool,
    pub avg_duration: &'static str,
    pub creator: &'static str,
    pub long_description: &'static str,
    pub media: GameMedia,
}

pub fn game_media(name: &str) -> GameMedia {
    match name {
        "tic_tac_toe" => GameMedia {
            accent_gradient: "from-primary-container/50 via-surface-container-low to-background",
            icon_emoji: "⭕",
        },
        "checkers" => GameMedia {
            accent_gradient: "from-tertiary-container/40 via-surface-container-low to-background",
            icon_emoji: "♟️",
        },
        "chess" => GameMedia {
            accent_gradient: "from-surface-container-high via-primary-container/30 to-background",
            icon_emoji: "♔",
        },
        "connect_four" => GameMedia {
            accent_gradient: "from-tertiary/40 via-secondary-container/30 to-background",
            icon_emoji: "🔴",
        },
        "backgammon" => GameMedia {
            accent_gradient: "from-secondary-container/50 via-tertiary-container/30 to-background",
            icon_emoji: "🎲",
        },
        "go" => GameMedia {
            accent_gradient: "from-surface-container via-primary-container/20 to-background",
            icon_emoji: "⚫",
        },
        "reversi" => GameMedia {
            accent_gradient: "from-primary/25 via-tertiary-container/40 to-background",
            icon_emoji: "⚪",
        },
        "catan" => GameMedia {
            accent_gradient: "from-tertiary/35 via-secondary-container/40 to-background",
            icon_emoji: "🏝️",
        },
        "monopoly" => GameMedia {
            accent_gradient: "from-primary-container/45 via-tertiary-container/30 to-background",
            icon_emoji: "💰",
        },
        "risk" => GameMedia {
            accent_gradient: "from-secondary/30 via-surface-container-high to-background",
            icon_emoji: "🌍",
        },
        "scrabble" => GameMedia {
            accent_gradient: "from-surface-container via-primary-container/25 to-background",
            icon_emoji: "🔤",
        },
        "chinese_checkers" => GameMedia {
            accent_gradient: "from-tertiary-container/45 via-primary/20 to-background",
            icon_emoji: "⭐",
        },
        "mahjong" => GameMedia {
            accent_gradient: "from-primary/30 via-secondary-container/35 to-background",
            icon_emoji: "🀄",
        },
        _ => GameMedia {
            accent_gradient: "from-secondary-container/30 via-surface-container-low to-background",
            icon_emoji: "🎮",
        },
    }
}

pub fn game_stub(name: &str) -> GameStubMeta {
    let media = game_media(name);
    match name {
        "tic_tac_toe" => GameStubMeta {
            tags: &["Strategy", "Classic"],
            active_players: 48,
            ping_ms: 18,
            featured: true,
            avg_duration: "5 – 12 min",
            creator: "IPEL GameDev",
            long_description: "Classic N×N tic-tac-toe with configurable board size and win length. Perfect for quick matches between friends in the lobby.",
            media,
        },
        "checkers" => GameStubMeta {
            tags: &["Board", "Turn-based"],
            active_players: 22,
            ping_ms: 24,
            featured: false,
            avg_duration: "10 – 20 min",
            creator: "IPEL GameDev",
            long_description: "Traditional checkers with WASM-powered logic. Claim seats in a lobby and play head-to-head.",
            media,
        },
        "chess" => GameStubMeta {
            tags: &["Classic", "1v1", "Ranked"],
            active_players: 36,
            ping_ms: 20,
            featured: false,
            avg_duration: "15 – 45 min",
            creator: "IPEL GameDev",
            long_description: "Standard chess with lobby timers and move validation. From blitz rooms to long classical sessions.",
            media,
        },
        "connect_four" => GameStubMeta {
            tags: &["Party", "Quick", "2P"],
            active_players: 18,
            ping_ms: 16,
            featured: false,
            avg_duration: "3 – 8 min",
            creator: "Community",
            long_description: "Drop discs, connect four in a row. Perfect filler between longer matches.",
            media,
        },
        "backgammon" => GameStubMeta {
            tags: &["Dice", "Classic", "2P"],
            active_players: 14,
            ping_ms: 28,
            featured: false,
            avg_duration: "12 – 25 min",
            creator: "Community",
            long_description: "Race your checkers home before your opponent. Doubling cube optional in lobby config.",
            media,
        },
        "go" => GameStubMeta {
            tags: &["Abstract", "Deep", "2P"],
            active_players: 11,
            ping_ms: 22,
            featured: false,
            avg_duration: "20 – 60 min",
            creator: "Community",
            long_description: "Ancient territory game on 9×9, 13×13, or 19×19 boards. Handicap stones supported.",
            media,
        },
        "reversi" => GameStubMeta {
            tags: &["Strategy", "Flip", "2P"],
            active_players: 9,
            ping_ms: 19,
            featured: false,
            avg_duration: "5 – 15 min",
            creator: "Community",
            long_description: "Othello-style disc flipping. Control the corners to dominate the board.",
            media,
        },
        "catan" => GameStubMeta {
            tags: &["Party", "Trading", "3–4P"],
            active_players: 52,
            ping_ms: 21,
            featured: false,
            avg_duration: "45 – 90 min",
            creator: "Community",
            long_description: "Trade wood, brick, and sheep to build roads, settlements, and cities. First to 10 victory points wins the lobby.",
            media,
        },
        "monopoly" => GameStubMeta {
            tags: &["Party", "Classic", "2–8P"],
            active_players: 44,
            ping_ms: 23,
            featured: false,
            avg_duration: "60 – 120 min",
            creator: "Community",
            long_description: "Buy properties, collect rent, and bankrupt your friends. House rules configurable before launch.",
            media,
        },
        "risk" => GameStubMeta {
            tags: &["Strategy", "Conquest", "2–6P"],
            active_players: 31,
            ping_ms: 26,
            featured: false,
            avg_duration: "90 – 180 min",
            creator: "Community",
            long_description: "Deploy armies, forge alliances, and conquer the world map. Elimination or mission cards in lobby config.",
            media,
        },
        "scrabble" => GameStubMeta {
            tags: &["Words", "Family", "2–4P"],
            active_players: 27,
            ping_ms: 18,
            featured: false,
            avg_duration: "30 – 60 min",
            creator: "Community",
            long_description: "Form high-scoring words on the shared board. Dictionary validation and tile bag synced server-side.",
            media,
        },
        "chinese_checkers" => GameStubMeta {
            tags: &["Party", "Hops", "2–6P"],
            active_players: 33,
            ping_ms: 17,
            featured: false,
            avg_duration: "15 – 30 min",
            creator: "Community",
            long_description: "Race marbles across the star board. Supports two to six players with team modes in the lobby.",
            media,
        },
        "mahjong" => GameStubMeta {
            tags: &["Tiles", "4P", "Classic"],
            active_players: 38,
            ping_ms: 20,
            featured: false,
            avg_duration: "40 – 80 min",
            creator: "Community",
            long_description: "Four-player tile melds, pungs, and chows. Riichi or classical scoring selectable per lobby.",
            media,
        },
        _ => GameStubMeta {
            tags: &["Multiplayer"],
            active_players: 12,
            ping_ms: 32,
            featured: false,
            avg_duration: "15 – 25 min",
            creator: "Community",
            long_description: "A published game on this server. Open a lobby to configure rules and invite players.",
            media,
        },
    }
}

#[derive(Clone, Debug)]
pub struct LeaderboardRow {
    pub rank: u32,
    pub player: String,
    pub score: i32,
    pub wins: u32,
    pub win_rate_pct: u32,
}

pub fn leaderboard_stub(game_name: &str) -> Vec<LeaderboardRow> {
    let _ = game_name;
    vec![
        LeaderboardRow { rank: 1, player: "NovaPilot".into(), score: 2840, wins: 42, win_rate_pct: 78 },
        LeaderboardRow { rank: 2, player: "CipherFox".into(), score: 2510, wins: 38, win_rate_pct: 71 },
        LeaderboardRow { rank: 3, player: "Guest".into(), score: 1920, wins: 29, win_rate_pct: 62 },
        LeaderboardRow { rank: 4, player: "ByteRunner".into(), score: 1650, wins: 24, win_rate_pct: 58 },
        LeaderboardRow { rank: 5, player: "GridLock".into(), score: 1400, wins: 19, win_rate_pct: 51 },
    ]
}

#[derive(Clone, Debug)]
pub struct SessionLogRow {
    pub match_id: String,
    pub outcome: String,
    pub winner: Option<String>,
    pub participants: u32,
    pub duration: String,
    pub state: &'static str,
}

pub fn session_log_stub(game_name: &str) -> Vec<SessionLogRow> {
    let prefix = game_name.chars().take(3).collect::<String>().to_uppercase();
    vec![
        SessionLogRow {
            match_id: format!("#{prefix}-88219"),
            outcome: "Victory".into(),
            winner: Some("NovaPilot".into()),
            participants: 4,
            duration: "18m 42s".into(),
            state: "Finished",
        },
        SessionLogRow {
            match_id: format!("#{prefix}-88104"),
            outcome: "Draw".into(),
            winner: None,
            participants: 2,
            duration: "9m 11s".into(),
            state: "Finished",
        },
        SessionLogRow {
            match_id: format!("#{prefix}-88077"),
            outcome: "Victory".into(),
            winner: Some("CipherFox".into()),
            participants: 2,
            duration: "14m 05s".into(),
            state: "Finished",
        },
    ]
}

#[derive(Clone, Debug)]
pub struct NotificationItem {
    pub title: String,
    pub body: String,
    pub time: &'static str,
    pub unread: bool,
}

pub fn notifications_stub() -> Vec<NotificationItem> {
    vec![
        NotificationItem {
            title: "Lobby started".into(),
            body: "Your room is now in-game.".into(),
            time: "2m ago",
            unread: true,
        },
        NotificationItem {
            title: "New game published".into(),
            body: "Tic Tac Toe v1.0.0 is live on this server.".into(),
            time: "1h ago",
            unread: true,
        },
        NotificationItem {
            title: "Developer access".into(),
            body: "Upload console is available for your account.".into(),
            time: "Yesterday",
            unread: false,
        },
    ]
}

#[derive(Clone, Debug, PartialEq)]
pub struct ActivityEvent {
    pub avatar_seed: String,
    pub actor: String,
    pub action: &'static str,
    pub target: String,
    pub time: &'static str,
}

pub fn activity_feed_stub() -> Vec<ActivityEvent> {
    vec![
        ActivityEvent { avatar_seed: "NovaPilot".into(), actor: "NovaPilot".into(), action: "won", target: "Tic Tac Toe #TIC-88219".into(), time: "2m ago" },
        ActivityEvent { avatar_seed: "CipherFox".into(), actor: "CipherFox".into(), action: "joined lobby", target: "Open Room #a3f2".into(), time: "5m ago" },
        ActivityEvent { avatar_seed: "ByteRunner".into(), actor: "ByteRunner".into(), action: "published", target: "Checkers v0.2".into(), time: "12m ago" },
        ActivityEvent { avatar_seed: "GridLock".into(), actor: "GridLock".into(), action: "created lobby", target: "Strategy Night".into(), time: "18m ago" },
        ActivityEvent { avatar_seed: "Guest".into(), actor: "Guest".into(), action: "claimed seat", target: "Lobby #b91c".into(), time: "24m ago" },
        ActivityEvent { avatar_seed: "NovaPilot".into(), actor: "NovaPilot".into(), action: "started match", target: "Tic Tac Toe".into(), time: "31m ago" },
        ActivityEvent { avatar_seed: "system".into(), actor: "System".into(), action: "deployed", target: "tic_tac_toe v1.0.0".into(), time: "1h ago" },
        ActivityEvent { avatar_seed: "CipherFox".into(), actor: "CipherFox".into(), action: "finished", target: "Checkers #CHK-88077".into(), time: "1h ago" },
    ]
}

#[derive(Clone, Debug)]
pub struct Badge {
    pub id: &'static str,
    pub label: &'static str,
    pub tier: &'static str,
    pub locked: bool,
}

pub fn badges_stub() -> Vec<Badge> {
    vec![
        Badge { id: "veteran", label: "Veteran", tier: "Gold", locked: false },
        Badge { id: "publisher", label: "Publisher", tier: "Silver", locked: false },
        Badge { id: "streak", label: "Win Streak", tier: "Bronze", locked: false },
        Badge { id: "elite", label: "Elite Operator", tier: "Elite", locked: true },
        Badge { id: "architect", label: "Architect", tier: "Elite", locked: true },
        Badge { id: "mentor", label: "Mentor", tier: "Gold", locked: true },
    ]
}

#[derive(Clone, Debug)]
pub struct KpiTrend {
    pub label: &'static str,
    pub value: String,
    pub delta_pct: &'static str,
    pub up: bool,
}

pub fn kpi_trends_stub() -> Vec<KpiTrend> {
    vec![
        KpiTrend { label: "Active sessions", value: "47".into(), delta_pct: "+12%", up: true },
        KpiTrend { label: "Published versions", value: "3".into(), delta_pct: "+1", up: true },
        KpiTrend { label: "Health score", value: "99.2%".into(), delta_pct: "+0.3%", up: true },
    ]
}

#[derive(Clone, Debug)]
pub struct ServerStatus {
    pub label: &'static str,
    pub ping_ms: u32,
    pub load_pct: u8,
}

pub fn server_status_stub() -> ServerStatus {
    ServerStatus {
        label: "System Stable",
        ping_ms: 18,
        load_pct: 34,
    }
}

#[derive(Clone, Debug)]
pub struct DeploymentRow {
    pub id: String,
    pub game_name: String,
    pub version: String,
    pub status: &'static str,
    pub deployed_at: &'static str,
}

pub fn deployment_rows_stub() -> Vec<DeploymentRow> {
    vec![
        DeploymentRow { id: "dep-001".into(), game_name: "tic_tac_toe".into(), version: "1.0.0".into(), status: "Live", deployed_at: "2h ago" },
        DeploymentRow { id: "dep-002".into(), game_name: "checkers".into(), version: "0.2.1".into(), status: "Live", deployed_at: "1d ago" },
        DeploymentRow { id: "dep-003".into(), game_name: "tic_tac_toe".into(), version: "0.9.0".into(), status: "Archived", deployed_at: "3d ago" },
    ]
}

#[derive(Clone, Debug)]
pub struct ApiTokenRow {
    pub id: String,
    pub label: String,
    pub masked_key: String,
    pub created_at: &'static str,
}

pub fn api_tokens_stub() -> Vec<ApiTokenRow> {
    vec![
        ApiTokenRow { id: "tok-1".into(), label: "CI Pipeline".into(), masked_key: "ipel_••••••••4f2a".into(), created_at: "Mar 12" },
        ApiTokenRow { id: "tok-2".into(), label: "Local Dev".into(), masked_key: "ipel_••••••••9b1c".into(), created_at: "Feb 28" },
    ]
}

pub const CLI_COMMANDS: &[(&str, &str)] = &[
    ("Upload game", "gamedev upload ./release.zip"),
    ("Validate draft", "gamedev validate --draft <id>"),
    ("Publish", "gamedev publish --name tic_tac_toe --version 1.0.0"),
];

#[derive(Clone, Debug)]
pub struct ProfileStub {
    pub display_name: String,
    pub rank_label: &'static str,
    pub rank_progress_pct: u8,
    pub matches_played: u32,
    pub matches_target: u32,
    pub upvotes: u32,
    pub upvotes_target: u32,
    pub stability_pct: f32,
    pub games_published: u32,
    pub total_downloads: u32,
    pub verified: bool,
    pub rep_score: u32,
}

pub fn profile_stub(display_name: &str) -> ProfileStub {
    ProfileStub {
        display_name: display_name.to_string(),
        rank_label: "Veteran II",
        rank_progress_pct: 65,
        matches_played: 142,
        matches_target: 200,
        upvotes: 89,
        upvotes_target: 100,
        stability_pct: 99.2,
        games_published: 2,
        total_downloads: 1840,
        verified: true,
        rep_score: 2840,
    }
}

pub fn lobby_elapsed_stub(created_at: i64) -> String {
    let now = js_now_secs();
    let secs = (now - created_at).max(0);
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else {
        format!("{}h", secs / 3600)
    }
}

pub fn estimated_match_time_stub() -> &'static str {
    "~8 min"
}

fn js_now_secs() -> i64 {
    (js_sys::Date::now() / 1000.0) as i64
}

pub fn ping_status_class(ms: u32) -> &'static str {
    if ms < 40 {
        "text-tertiary"
    } else if ms < 100 {
        "text-secondary"
    } else {
        "text-error"
    }
}

pub fn pro_tip_stub() -> &'static str {
    "Use Launch Game in the sidebar to create a room instantly. Pick your game type and invite friends from the lobby browser."
}
