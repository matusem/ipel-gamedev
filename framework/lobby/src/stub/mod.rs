//! Demo-mode synthetic API and cosmetic fallbacks (gradients, placeholder art).

pub mod demo_api;
pub mod demo_images;
pub mod demo_mode;

#[derive(Clone, Debug)]
pub struct GameMedia {
    pub accent_gradient: &'static str,
    pub icon_emoji: &'static str,
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

/// Elapsed time since lobby `created_at` (unix seconds).
pub fn lobby_elapsed_stub(created_at: i64) -> String {
    let now = (js_sys::Date::now() / 1000.0) as i64;
    let secs = (now - created_at).max(0);
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else {
        format!("{}h", secs / 3600)
    }
}
