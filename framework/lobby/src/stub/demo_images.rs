//! Verified image URLs for demo storefront screenshots (Unsplash).

pub fn screenshots_for_game(game: &str) -> Vec<(&'static str, &'static str, &'static str)> {
    match game {
        "tic_tac_toe" => tic_tac_toe_screenshots(),
        "checkers" => checkers_screenshots(),
        "chess" => chess_screenshots(),
        "connect_four" => connect_four_screenshots(),
        "backgammon" => backgammon_screenshots(),
        "go" => go_screenshots(),
        "reversi" => reversi_screenshots(),
        "catan" => catan_screenshots(),
        "monopoly" => monopoly_screenshots(),
        "risk" => risk_screenshots(),
        "scrabble" => scrabble_screenshots(),
        "chinese_checkers" => chinese_checkers_screenshots(),
        "mahjong" => mahjong_screenshots(),
        _ => generic_game_screenshots(),
    }
}

pub fn cover_image_url(game: &str) -> Option<&'static str> {
    screenshots_for_game(game)
        .first()
        .map(|(_, _, url)| *url)
}

pub fn tic_tac_toe_screenshots() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        (
            "s1",
            "Tic-tac-toe on a wooden shelf",
            "https://images.unsplash.com/photo-1773101883545-a7330245c3f1?w=1600&auto=format&fit=crop&q=85",
        ),
        (
            "s2",
            "Game board with X and O pieces",
            "https://images.unsplash.com/photo-1773101883585-4e89e20c7321?w=1600&auto=format&fit=crop&q=85",
        ),
        (
            "s3",
            "Travel tic-tac-toe kit",
            "https://images.unsplash.com/photo-1600224374823-211f85c16521?w=1600&auto=format&fit=crop&q=85",
        ),
    ]
}

pub fn checkers_screenshots() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        (
            "s1",
            "Red and black checker pieces",
            "https://images.unsplash.com/photo-1610232826230-e5e6c6a1efef?w=1600&auto=format&fit=crop&q=85",
        ),
        (
            "s2",
            "Oversized checkers on the board",
            "https://images.unsplash.com/photo-1539191123335-3ebecae7a6ad?w=1600&auto=format&fit=crop&q=85",
        ),
        (
            "s3",
            "Checkered board close-up",
            "https://images.unsplash.com/photo-1551198581-aec5c1556d7c?w=1600&auto=format&fit=crop&q=85",
        ),
        (
            "s4",
            "Players at the checkers table",
            "https://images.unsplash.com/photo-1644010086037-ac050b0f8e44?w=1600&auto=format&fit=crop&q=85",
        ),
    ]
}

pub fn chess_screenshots() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        (
            "s1",
            "Chess pieces on the board",
            "https://images.unsplash.com/photo-1677816155981-919b9a6eeded?w=1600&auto=format&fit=crop&q=85",
        ),
        (
            "s2",
            "Chessboard ready to play",
            "https://images.unsplash.com/photo-1761393654131-c17830b2f823?w=1600&auto=format&fit=crop&q=85",
        ),
        (
            "s3",
            "Close-up of carved pieces",
            "https://images.unsplash.com/photo-1578662996442-48f60103fc96?w=1600&auto=format&fit=crop&q=85",
        ),
        (
            "s4",
            "Park chess match",
            "https://images.unsplash.com/photo-1529699211952-734e80c4d42b?w=1600&auto=format&fit=crop&q=85",
        ),
        (
            "s5",
            "Tournament clock and board",
            "https://images.unsplash.com/photo-1586165368502-1bad197a6461?w=1600&auto=format&fit=crop&q=85",
        ),
    ]
}

pub fn connect_four_screenshots() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        (
            "s1",
            "Connect Four discs in the grid",
            "https://images.unsplash.com/photo-1729856964184-4b6ef5e2c969?w=1600&auto=format&fit=crop&q=85",
        ),
        (
            "s2",
            "Giant connect four on the patio",
            "https://images.unsplash.com/photo-1768058239203-27c0f41bbdcb?w=1600&auto=format&fit=crop&q=85",
        ),
        (
            "s3",
            "Colorful board game night",
            "https://images.unsplash.com/photo-1611194024022-3cea3145911c?w=1600&auto=format&fit=crop&q=85",
        ),
    ]
}

pub fn backgammon_screenshots() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        (
            "s1",
            "Backgammon board ready on the table",
            "https://images.unsplash.com/photo-1748130110932-a15df790f831?w=1600&auto=format&fit=crop&q=85",
        ),
        (
            "s2",
            "Backgammon set outdoors",
            "https://images.unsplash.com/photo-1743571029574-e70b76511c78?w=1600&auto=format&fit=crop&q=85",
        ),
        (
            "s3",
            "Wooden game table detail",
            "https://images.unsplash.com/photo-1573844389110-72746a45efbc?w=1600&auto=format&fit=crop&q=85",
        ),
    ]
}

pub fn go_screenshots() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        (
            "s1",
            "Go stones on a wooden board",
            "https://images.unsplash.com/photo-1774234528903-f520d964ba13?w=1600&auto=format&fit=crop&q=85",
        ),
        (
            "s2",
            "Stone bowls on a go board",
            "https://images.unsplash.com/photo-1743055838273-06d4bc8bca74?w=1600&auto=format&fit=crop&q=85",
        ),
        (
            "s3",
            "Players mid-game on 19×19",
            "https://images.unsplash.com/photo-1777652907869-97332fd656d8?w=1600&auto=format&fit=crop&q=85",
        ),
    ]
}

pub fn reversi_screenshots() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        (
            "s1",
            "Black and white discs on felt",
            "https://images.unsplash.com/photo-1551198581-aec5c1556d7c?w=1600&auto=format&fit=crop&q=85",
        ),
        (
            "s2",
            "Strategy board close-up",
            "https://images.unsplash.com/photo-1642056446459-1f10774273f2?w=1600&auto=format&fit=crop&q=85",
        ),
        (
            "s3",
            "Tabletop game night",
            "https://images.unsplash.com/photo-1611194024022-3cea3145911c?w=1600&auto=format&fit=crop&q=85",
        ),
    ]
}

pub fn catan_screenshots() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        (
            "s1",
            "Catan board mid-game",
            "https://images.unsplash.com/photo-1667118398882-fe8fd62665c6?w=1600&auto=format&fit=crop&q=85",
        ),
        (
            "s2",
            "Stack of board games including Catan",
            "https://images.unsplash.com/photo-1769288361254-abb4783a6070?w=1600&auto=format&fit=crop&q=85",
        ),
        (
            "s3",
            "Friends around the table",
            "https://images.unsplash.com/photo-1611194024022-3cea3145911c?w=1600&auto=format&fit=crop&q=85",
        ),
    ]
}

pub fn monopoly_screenshots() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        (
            "s1",
            "Monopoly night with friends",
            "https://images.unsplash.com/photo-1677188010559-0667a1ed33a0?w=1600&auto=format&fit=crop&q=85",
        ),
        (
            "s2",
            "Board game with dice",
            "https://images.unsplash.com/photo-1642056446459-1f10774273f2?w=1600&auto=format&fit=crop&q=85",
        ),
        (
            "s3",
            "Property cards and tokens",
            "https://images.unsplash.com/photo-1511895426328-dc8714191300?w=1600&auto=format&fit=crop&q=85",
        ),
    ]
}

pub fn risk_screenshots() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        (
            "s1",
            "World map strategy board",
            "https://images.unsplash.com/photo-1607472586893-edb57bdc0e39?w=1600&auto=format&fit=crop&q=85",
        ),
        (
            "s2",
            "Tactical map close-up",
            "https://images.unsplash.com/photo-1585504198199-20277593b94f?w=1600&auto=format&fit=crop&q=85",
        ),
        (
            "s3",
            "Long campaign at the table",
            "https://images.unsplash.com/photo-1573844389110-72746a45efbc?w=1600&auto=format&fit=crop&q=85",
        ),
    ]
}

pub fn scrabble_screenshots() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        (
            "s1",
            "Scrabble tiles on the board",
            "https://images.unsplash.com/photo-1671628586515-0e4d9456f291?w=1600&auto=format&fit=crop&q=85",
        ),
        (
            "s2",
            "Letter tiles on wood",
            "https://images.unsplash.com/photo-1704969724000-154d4fb94344?w=1600&auto=format&fit=crop&q=85",
        ),
        (
            "s3",
            "Word-building session",
            "https://images.unsplash.com/photo-1642406415849-a410b5d01a94?w=1600&auto=format&fit=crop&q=85",
        ),
    ]
}

pub fn chinese_checkers_screenshots() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        (
            "s1",
            "Star board with marbles",
            "https://images.unsplash.com/photo-1511895426328-dc8714191300?w=1600&auto=format&fit=crop&q=85",
        ),
        (
            "s2",
            "Colorful pegs on the board",
            "https://images.unsplash.com/photo-1606092195730-5d7b9af1efc5?w=1600&auto=format&fit=crop&q=85",
        ),
        (
            "s3",
            "Six-player hop strategy",
            "https://images.unsplash.com/photo-1729856964184-4b6ef5e2c969?w=1600&auto=format&fit=crop&q=85",
        ),
    ]
}

pub fn mahjong_screenshots() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        (
            "s1",
            "Mahjong tiles laid out",
            "https://images.unsplash.com/photo-1585504198199-20277593b94f?w=1600&auto=format&fit=crop&q=85",
        ),
        (
            "s2",
            "Tile wall and discards",
            "https://images.unsplash.com/photo-1642056446459-1f10774273f2?w=1600&auto=format&fit=crop&q=85",
        ),
        (
            "s3",
            "Four-player table setup",
            "https://images.unsplash.com/photo-1611194024022-3cea3145911c?w=1600&auto=format&fit=crop&q=85",
        ),
    ]
}

pub fn generic_game_screenshots() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        (
            "s1",
            "Board game on the table",
            "https://images.unsplash.com/photo-1573844389110-72746a45efbc?w=1600&auto=format&fit=crop&q=85",
        ),
        (
            "s2",
            "Wooden strategy set",
            "https://images.unsplash.com/photo-1728939862852-4470c2604c9c?w=1600&auto=format&fit=crop&q=85",
        ),
        (
            "s3",
            "Game night with friends",
            "https://images.unsplash.com/photo-1611194024022-3cea3145911c?w=1600&auto=format&fit=crop&q=85",
        ),
    ]
}
