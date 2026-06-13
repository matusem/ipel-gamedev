use crate::db::{FinishedGameRow, GameInstanceStore};
use crate::platform_stats::compute_leaderboard;
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Screenshot {
    pub id: String,
    pub caption: String,
    pub gradient: String,
    #[serde(default)]
    pub image_url: Option<String>,
}

fn shot(id: &str, caption: &str, image_url: &str, gradient: &str) -> Screenshot {
    Screenshot {
        id: id.into(),
        caption: caption.into(),
        gradient: gradient.into(),
        image_url: Some(image_url.into()),
    }
}

fn tic_tac_toe_shots() -> Vec<Screenshot> {
    vec![
        shot(
            "1",
            "Tic-tac-toe on a wooden shelf",
            "https://images.unsplash.com/photo-1773101883545-a7330245c3f1?w=1600&auto=format&fit=crop&q=85",
            "from-primary-container/60 via-surface-container to-background",
        ),
        shot(
            "2",
            "Game board with X and O pieces",
            "https://images.unsplash.com/photo-1773101883585-4e89e20c7321?w=1600&auto=format&fit=crop&q=85",
            "from-tertiary-container/50 via-surface-container-low to-background",
        ),
        shot(
            "3",
            "Mid-game — crosses and noughts",
            "https://upload.wikimedia.org/wikipedia/commons/thumb/f/f6/Tic_Tac_Toe.png/1280px-Tic_Tac_Toe.png",
            "from-secondary-container/40 via-surface-container to-background",
        ),
        shot(
            "4",
            "Classic winning line",
            "https://upload.wikimedia.org/wikipedia/commons/thumb/3/32/Tic_tac_toe.svg/1280px-Tic_tac_toe.svg.png",
            "from-primary/30 via-surface-container to-background",
        ),
        shot(
            "5",
            "Travel tic-tac-toe kit",
            "https://images.unsplash.com/photo-1600224374823-211f85c16521?w=1600&auto=format&fit=crop&q=85",
            "from-tertiary/50 via-background to-surface-container-low",
        ),
    ]
}

fn checkers_shots() -> Vec<Screenshot> {
    vec![
        shot(
            "1",
            "Red and black checker pieces",
            "https://images.unsplash.com/photo-1610232826230-e5e6c6a1efef?w=1600&auto=format&fit=crop&q=85",
            "from-tertiary-container/50 via-surface-container to-background",
        ),
        shot(
            "2",
            "Oversized checkers on the board",
            "https://images.unsplash.com/photo-1539191123335-3ebecae7a6ad?w=1600&auto=format&fit=crop&q=85",
            "from-primary-container/40 via-surface-container-low to-background",
        ),
        shot(
            "3",
            "Checkered board close-up",
            "https://images.unsplash.com/photo-1551198581-aec5c1556d7c?w=1600&auto=format&fit=crop&q=85",
            "from-secondary-container/30 via-surface-container to-background",
        ),
        shot(
            "4",
            "Players at the checkers table",
            "https://images.unsplash.com/photo-1644010086037-ac050b0f8e44?w=1600&auto=format&fit=crop&q=85",
            "from-tertiary/40 via-surface-container to-background",
        ),
        shot(
            "5",
            "Vintage checkers collection",
            "https://upload.wikimedia.org/wikipedia/commons/f/f9/The_Childrens_Museum_of_Indianapolis_-_Checkers.jpg",
            "from-primary-container/30 via-surface-container-low to-background",
        ),
    ]
}

/// Old placeholder URLs from the first demo pass — replace on read.
const STALE_IMAGE_MARKERS: &[&str] = &[
    "Gomoku.jpg",
    "photo-1611194024022",
    "photo-1511512578047",
    "photo-1550745165",
    "photo-1529699211952",
    "photo-1606092195730",
    "Canadian_Checkers_gameboard",
    "Draughts.svg",
];

fn screenshots_need_refresh(shots: &[Screenshot]) -> bool {
    if shots.is_empty() {
        return true;
    }
    if shots
        .iter()
        .all(|s| s.image_url.as_deref().unwrap_or("").is_empty())
    {
        return true;
    }
    shots.iter().any(|s| {
        s.image_url
            .as_deref()
            .is_some_and(|u| STALE_IMAGE_MARKERS.iter().any(|m| u.contains(m)))
    })
}

fn merge_default_screenshots(game_name: &str, shots: &mut Vec<Screenshot>) {
    if screenshots_need_refresh(shots) {
        *shots = default_storefront(game_name).screenshots;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchNote {
    pub version: String,
    pub date: String,
    pub title: String,
    pub body: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AspectRatings {
    pub gameplay: f32,
    pub balance: f32,
    pub visuals: f32,
    pub social: f32,
    pub depth: f32,
}

impl Default for AspectRatings {
    fn default() -> Self {
        Self {
            gameplay: 4.0,
            balance: 4.0,
            visuals: 3.5,
            social: 4.0,
            depth: 3.5,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StorefrontRow {
    pub game_name: String,
    pub owner_user_id: Option<Uuid>,
    pub short_tagline: Option<String>,
    pub long_description: String,
    pub screenshots: Vec<Screenshot>,
    pub patch_notes: Vec<PatchNote>,
    pub tags: Vec<String>,
    pub avg_session_mins: i32,
    pub featured: bool,
    pub creator_display_name: Option<String>,
    pub updated_at: i64,
}

#[derive(Debug, Clone)]
pub struct ReviewRow {
    pub id: Uuid,
    pub game_name: String,
    pub user_id: Uuid,
    pub display_name: String,
    pub body: String,
    pub aspects: AspectRatings,
    pub helpful_votes: i32,
    pub created_at: i64,
}

#[derive(Debug, Clone)]
pub struct CommentRow {
    pub id: Uuid,
    pub game_name: String,
    pub user_id: Uuid,
    pub display_name: String,
    pub body: String,
    pub created_at: i64,
}

#[derive(Debug, Clone)]
pub struct PlayTimeEntry {
    pub display_name: String,
    pub total_mins: i32,
    pub sessions: u32,
}

pub fn default_cover_image(game_name: &str) -> Option<String> {
    default_storefront(game_name)
        .screenshots
        .first()
        .and_then(|s| s.image_url.clone())
}

fn default_storefront(game_name: &str) -> StorefrontRow {
    match game_name {
        "tic_tac_toe" => StorefrontRow {
            game_name: game_name.into(),
            owner_user_id: None,
            short_tagline: Some("Classic strategy — configurable board.".into()),
            long_description: "Classic N×N tic-tac-toe with configurable board size and win length. Claim a seat in a lobby, tune the rules, and challenge friends in real time. Built with WASM logic and a polished web UI.".into(),
            screenshots: tic_tac_toe_shots(),
            patch_notes: vec![
                PatchNote { version: "1.0.0".into(), date: "2026-04-01".into(), title: "Launch".into(), body: "Initial release with lobby config UI and WASM logic.".into(), tags: vec!["feature".into()] },
                PatchNote { version: "0.9.2".into(), date: "2026-03-18".into(), title: "Seat ready fix".into(), body: "Fixed ready-state desync when host changes game type.".into(), tags: vec!["bugfix".into()] },
            ],
            tags: vec!["Strategy".into(), "Classic".into(), "2P".into()],
            avg_session_mins: 8,
            featured: true,
            creator_display_name: Some("UPJŠ GDD Platform".into()),
            updated_at: GameInstanceStore::now_secs(),
        },
        "checkers" => StorefrontRow {
            game_name: game_name.into(),
            owner_user_id: None,
            short_tagline: Some("Head-to-head board combat.".into()),
            long_description: "Traditional checkers with WASM-powered move validation. Supports lobby staging, seat claims, and live chat while you wait for your opponent.".into(),
            screenshots: checkers_shots(),
            patch_notes: vec![
                PatchNote { version: "0.2.1".into(), date: "2026-03-22".into(), title: "Draw detection".into(), body: "Improved stalemate and repetition handling.".into(), tags: vec!["bugfix".into()] },
            ],
            tags: vec!["Board".into(), "Turn-based".into()],
            avg_session_mins: 15,
            featured: false,
            creator_display_name: Some("UPJŠ GDD Platform".into()),
            updated_at: GameInstanceStore::now_secs(),
        },
        _ => StorefrontRow {
            game_name: game_name.into(),
            owner_user_id: None,
            short_tagline: None,
            long_description: format!("Community game `{game_name}` on UPJŠ GDD Platform. Open a lobby to play with friends."),
            screenshots: vec![],
            patch_notes: vec![],
            tags: vec!["Multiplayer".into()],
            avg_session_mins: 12,
            featured: false,
            creator_display_name: None,
            updated_at: GameInstanceStore::now_secs(),
        },
    }
}

pub async fn ensure_storefront(
    pool: &SqlitePool,
    game_name: &str,
) -> Result<StorefrontRow, sqlx::Error> {
    if let Some(row) = get_storefront(pool, game_name).await? {
        return Ok(row);
    }
    let def = default_storefront(game_name);
    insert_storefront(pool, &def).await?;
    if demo_seed_enabled() {
        seed_reviews_and_comments(pool, game_name).await?;
    }
    Ok(def)
}

pub fn demo_seed_enabled() -> bool {
    std::env::var("SEED_DEMO_CONTENT")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

pub async fn seed_demo_storefront_content(
    pool: &SqlitePool,
    game_name: &str,
) -> Result<(), sqlx::Error> {
    let _ = ensure_storefront(pool, game_name).await?;
    seed_reviews_and_comments(pool, game_name).await
}

async fn insert_storefront(pool: &SqlitePool, s: &StorefrontRow) -> Result<(), sqlx::Error> {
    let shots = serde_json::to_string(&s.screenshots).unwrap_or_else(|_| "[]".into());
    let patches = serde_json::to_string(&s.patch_notes).unwrap_or_else(|_| "[]".into());
    let tags = serde_json::to_string(&s.tags).unwrap_or_else(|_| "[]".into());
    sqlx::query(
        "INSERT INTO game_storefront (game_name, owner_user_id, short_tagline, long_description, screenshots_json, patch_notes_json, tags_json, avg_session_mins, featured, creator_display_name, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&s.game_name)
    .bind(s.owner_user_id.map(|u| u.to_string()))
    .bind(&s.short_tagline)
    .bind(&s.long_description)
    .bind(shots)
    .bind(patches)
    .bind(tags)
    .bind(s.avg_session_mins)
    .bind(if s.featured { 1 } else { 0 })
    .bind(&s.creator_display_name)
    .bind(s.updated_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_storefront(
    pool: &SqlitePool,
    game_name: &str,
) -> Result<Option<StorefrontRow>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT game_name, owner_user_id, short_tagline, long_description, screenshots_json, patch_notes_json, tags_json, avg_session_mins, featured, creator_display_name, updated_at FROM game_storefront WHERE game_name = ?",
    )
    .bind(game_name)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| {
        let owner: Option<String> = r.get(1);
        let shots: String = r.get(4);
        let patches: String = r.get(5);
        let tags: String = r.get(6);
        let game_name: String = r.get(0);
        let mut screenshots: Vec<Screenshot> = serde_json::from_str(&shots).unwrap_or_default();
        merge_default_screenshots(&game_name, &mut screenshots);
        StorefrontRow {
            game_name,
            owner_user_id: owner.and_then(|s| Uuid::parse_str(&s).ok()),
            short_tagline: r.get(2),
            long_description: r.get(3),
            screenshots,
            patch_notes: serde_json::from_str(&patches).unwrap_or_default(),
            tags: serde_json::from_str(&tags).unwrap_or_default(),
            avg_session_mins: r.get(7),
            featured: r.get::<i32, _>(8) != 0,
            creator_display_name: r.get(9),
            updated_at: r.get(10),
        }
    }))
}

pub async fn update_storefront(
    pool: &SqlitePool,
    game_name: &str,
    short_tagline: Option<String>,
    long_description: String,
    screenshots_json: &str,
    patch_notes_json: &str,
    tags_json: &str,
    avg_session_mins: i32,
) -> Result<bool, sqlx::Error> {
    let now = GameInstanceStore::now_secs();
    let res = sqlx::query(
        "UPDATE game_storefront SET short_tagline = ?, long_description = ?, screenshots_json = ?, patch_notes_json = ?, tags_json = ?, avg_session_mins = ?, updated_at = ? WHERE game_name = ?",
    )
    .bind(short_tagline)
    .bind(long_description)
    .bind(screenshots_json)
    .bind(patch_notes_json)
    .bind(tags_json)
    .bind(avg_session_mins)
    .bind(now)
    .bind(game_name)
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn user_can_edit_storefront(
    pool: &SqlitePool,
    user_id: Uuid,
    game_name: &str,
) -> Result<bool, sqlx::Error> {
    let row = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM game_drafts WHERE owner_user_id = ? AND game_name = ? AND status IN ('ready', 'published')",
    )
    .bind(user_id.to_string())
    .bind(game_name)
    .fetch_one(pool)
    .await?;
    Ok(row > 0)
}

async fn seed_reviews_and_comments(pool: &SqlitePool, game_name: &str) -> Result<(), sqlx::Error> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM game_reviews WHERE game_name = ?")
        .bind(game_name)
        .fetch_one(pool)
        .await?;
    if count > 0 {
        return Ok(());
    }
    let now = GameInstanceStore::now_secs();
    let reviews = [
        (
            "NovaPilot",
            "Solid quick matches. Config UI is clean.",
            AspectRatings {
                gameplay: 5.0,
                balance: 4.0,
                visuals: 4.0,
                social: 5.0,
                depth: 3.0,
            },
        ),
        (
            "CipherFox",
            "Great for lunch-break games with friends.",
            AspectRatings {
                gameplay: 4.0,
                balance: 4.5,
                visuals: 3.5,
                social: 4.5,
                depth: 3.5,
            },
        ),
        (
            "ByteRunner",
            "Would love more board sizes in ranked mode.",
            AspectRatings {
                gameplay: 4.5,
                balance: 3.5,
                visuals: 4.0,
                social: 4.0,
                depth: 4.0,
            },
        ),
    ];
    for (i, (name, body, aspects)) in reviews.iter().enumerate() {
        let aspects_json = serde_json::to_string(aspects).unwrap_or_default();
        sqlx::query(
            "INSERT INTO game_reviews (id, game_name, user_id, display_name, body, aspects_json, helpful_votes, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(game_name)
        .bind(Uuid::new_v4().to_string())
        .bind(*name)
        .bind(*body)
        .bind(aspects_json)
        .bind((12 - i as i32) * 3)
        .bind(now - (i as i64) * 86_400)
        .execute(pool)
        .await?;
    }
    let comments = [
        ("GridLock", "Anyone up for a 5x5 win-4 lobby tonight?"),
        ("Guest", "First win — loving the seat/ready flow."),
        (
            "NovaPilot",
            "Dev: changelog for 1.0.0 is accurate, nice polish.",
        ),
    ];
    for (i, (name, body)) in comments.iter().enumerate() {
        sqlx::query(
            "INSERT INTO game_comments (id, game_name, user_id, display_name, body, created_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(game_name)
        .bind(Uuid::new_v4().to_string())
        .bind(*name)
        .bind(*body)
        .bind(now - (i as i64) * 3600)
        .execute(pool)
        .await?;
    }
    Ok(())
}

pub async fn list_reviews(
    pool: &SqlitePool,
    game_name: &str,
    limit: i64,
) -> Result<Vec<ReviewRow>, sqlx::Error> {
    let _ = ensure_storefront(pool, game_name).await?;
    let rows = sqlx::query(
        "SELECT id, game_name, user_id, display_name, body, aspects_json, helpful_votes, created_at FROM game_reviews WHERE game_name = ? ORDER BY created_at DESC LIMIT ?",
    )
    .bind(game_name)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().filter_map(map_review).collect())
}

fn map_review(r: sqlx::sqlite::SqliteRow) -> Option<ReviewRow> {
    let id_s: String = r.get(0);
    let uid_s: String = r.get(2);
    let aspects: String = r.get(5);
    Some(ReviewRow {
        id: Uuid::parse_str(&id_s).ok()?,
        game_name: r.get(1),
        user_id: Uuid::parse_str(&uid_s).ok()?,
        display_name: r.get(3),
        body: r.get(4),
        aspects: serde_json::from_str(&aspects).unwrap_or_default(),
        helpful_votes: r.get(6),
        created_at: r.get(7),
    })
}

pub async fn list_comments(
    pool: &SqlitePool,
    game_name: &str,
    limit: i64,
) -> Result<Vec<CommentRow>, sqlx::Error> {
    let _ = ensure_storefront(pool, game_name).await?;
    let rows = sqlx::query(
        "SELECT id, game_name, user_id, display_name, body, created_at FROM game_comments WHERE game_name = ? ORDER BY created_at DESC LIMIT ?",
    )
    .bind(game_name)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|r| {
            Some(CommentRow {
                id: Uuid::parse_str(&r.get::<String, _>(0)).ok()?,
                game_name: r.get(1),
                user_id: Uuid::parse_str(&r.get::<String, _>(2)).ok()?,
                display_name: r.get(3),
                body: r.get(4),
                created_at: r.get(5),
            })
        })
        .collect())
}

pub async fn aggregate_aspect_ratings(
    pool: &SqlitePool,
    game_name: &str,
) -> Result<AspectRatings, sqlx::Error> {
    let reviews = list_reviews(pool, game_name, 100).await?;
    if reviews.is_empty() {
        return Ok(AspectRatings::default());
    }
    let n = reviews.len() as f32;
    let mut sum = AspectRatings::default();
    for r in &reviews {
        sum.gameplay += r.aspects.gameplay;
        sum.balance += r.aspects.balance;
        sum.visuals += r.aspects.visuals;
        sum.social += r.aspects.social;
        sum.depth += r.aspects.depth;
    }
    Ok(AspectRatings {
        gameplay: sum.gameplay / n,
        balance: sum.balance / n,
        visuals: sum.visuals / n,
        social: sum.social / n,
        depth: sum.depth / n,
    })
}

pub async fn submit_review(
    pool: &SqlitePool,
    game_name: &str,
    user_id: Uuid,
    display_name: &str,
    body: &str,
    aspects: &AspectRatings,
) -> Result<ReviewRow, sqlx::Error> {
    let _ = ensure_storefront(pool, game_name).await?;
    let id = Uuid::new_v4();
    let now = GameInstanceStore::now_secs();
    let aspects_json = serde_json::to_string(aspects).unwrap_or_default();
    sqlx::query(
        "INSERT INTO game_reviews (id, game_name, user_id, display_name, body, aspects_json, helpful_votes, created_at) VALUES (?, ?, ?, ?, ?, ?, 0, ?)",
    )
    .bind(id.to_string())
    .bind(game_name)
    .bind(user_id.to_string())
    .bind(display_name)
    .bind(body)
    .bind(aspects_json)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(ReviewRow {
        id,
        game_name: game_name.into(),
        user_id,
        display_name: display_name.into(),
        body: body.into(),
        aspects: aspects.clone(),
        helpful_votes: 0,
        created_at: now,
    })
}

pub async fn submit_comment(
    pool: &SqlitePool,
    game_name: &str,
    user_id: Uuid,
    display_name: &str,
    body: &str,
) -> Result<CommentRow, sqlx::Error> {
    let _ = ensure_storefront(pool, game_name).await?;
    let id = Uuid::new_v4();
    let now = GameInstanceStore::now_secs();
    sqlx::query(
        "INSERT INTO game_comments (id, game_name, user_id, display_name, body, created_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(id.to_string())
    .bind(game_name)
    .bind(user_id.to_string())
    .bind(display_name)
    .bind(body)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(CommentRow {
        id,
        game_name: game_name.into(),
        user_id,
        display_name: display_name.into(),
        body: body.into(),
        created_at: now,
    })
}

pub async fn user_voted_review(
    pool: &SqlitePool,
    review_id: Uuid,
    user_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM game_review_votes WHERE review_id = ? AND user_id = ?",
    )
    .bind(review_id.to_string())
    .bind(user_id.to_string())
    .fetch_one(pool)
    .await?;
    Ok(count > 0)
}

pub async fn mark_review_helpful(
    pool: &SqlitePool,
    review_id: Uuid,
    user_id: Uuid,
) -> Result<ReviewRow, sqlx::Error> {
    let now = GameInstanceStore::now_secs();
    let inserted = sqlx::query(
        "INSERT OR IGNORE INTO game_review_votes (review_id, user_id, created_at) VALUES (?, ?, ?)",
    )
    .bind(review_id.to_string())
    .bind(user_id.to_string())
    .bind(now)
    .execute(pool)
    .await?;
    if inserted.rows_affected() > 0 {
        sqlx::query("UPDATE game_reviews SET helpful_votes = helpful_votes + 1 WHERE id = ?")
            .bind(review_id.to_string())
            .execute(pool)
            .await?;
    }
    let row = sqlx::query(
        "SELECT id, game_name, user_id, display_name, body, aspects_json, helpful_votes, created_at FROM game_reviews WHERE id = ?",
    )
    .bind(review_id.to_string())
    .fetch_optional(pool)
    .await?;
    row.and_then(map_review)
        .ok_or_else(|| sqlx::Error::RowNotFound)
}

pub async fn list_featured_game_names(pool: &SqlitePool) -> Result<Vec<String>, sqlx::Error> {
    let rows = sqlx::query_scalar::<_, String>(
        "SELECT game_name FROM game_storefront WHERE featured = 1 ORDER BY updated_at DESC",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn catalog_meta_for_game(
    pool: &SqlitePool,
    game_name: &str,
) -> Result<(bool, Option<String>, Vec<String>, i32), sqlx::Error> {
    if let Some(sf) = get_storefront(pool, game_name).await? {
        return Ok((
            sf.featured,
            sf.creator_display_name,
            sf.tags,
            sf.avg_session_mins,
        ));
    }
    let def = default_storefront(game_name);
    Ok((
        def.featured,
        def.creator_display_name,
        def.tags,
        def.avg_session_mins,
    ))
}

pub fn compute_playtime_leaderboard(
    rows: &[FinishedGameRow],
    avg_session_mins: i32,
    limit: usize,
) -> Vec<PlayTimeEntry> {
    let lb = compute_leaderboard(rows, 200);
    let mut out: Vec<PlayTimeEntry> = lb
        .into_iter()
        .map(|e| PlayTimeEntry {
            display_name: e.display_name,
            sessions: e.games_played,
            total_mins: (e.games_played as i32) * avg_session_mins.max(1),
        })
        .collect();
    out.sort_by(|a, b| b.total_mins.cmp(&a.total_mins));
    out.truncate(limit);
    out
}
