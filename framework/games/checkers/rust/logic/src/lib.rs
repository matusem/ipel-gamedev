//! English / American **8×8 checkers** (`GameCore` implementation).
//!
//! ## Extension points
//! - [`Config`] — fixed rules today; add fields + `config_schema` in `manifest.json` if you need variants.
//! - [`MovePath`] — wire format for a full turn (simple move or multi-jump chain).
//! - [`Checkers::take_action`] — apply a validated path; keep capture / promotion rules in sync with [`legal_moves`].
//!
//! ## Rules (this variant)
//! - Playable cells: dark squares only, `(row + col) % 2 == 1`.
//! - **Dark** starts rows `0..2`, **Light** rows `6..8` (two rows each); Dark moves first. Men advance toward the
//!   **far rank** (Dark: increasing `row`, Light: decreasing `row`).
//! - **Kings** slide any distance along a diagonal through empty squares; **men** move one diagonal step forward.
//! - **Captures are optional**: simple (non-capture) moves are always allowed when the piece can legally move.
//! - If **any** capture exists and the player plays a **non-capture** move: if the **moved** piece could have
//!   captured, it is **removed**; if it **could not** capture, exactly **one** piece that **could** (among
//!   max-capture leaders) is removed — **king** before **man**, then **most** captures from that square, then
//!   deterministic tie-break.
//! - **Capture length**: if the player captures, the path must remove the **maximum** number of pieces among
//!   all capture sequences available to that side; if several paths tie, the player may choose.
//! - **King capture priority**: if a king can participate in such a maximum-capture path, the player may not
//!   start a maximum-capture path with a man; only king-led max captures are legal among captures.
//! - **Flying king captures**: along a diagonal jump, any number of empty squares may sit before the jumped
//!   enemy and before the landing square.
//!
//! ## Game over
//! - Side to move has **no legal moves**, or **no pieces** → opponent wins.

use game::{Action, Config as GameConfig, GameCore, PlayerState as GamePlayerStateTrait};
use serde::de::{self, Deserializer, MapAccess, Visitor};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Serialize, Deserialize, Debug)]
pub struct Checkers;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Player {
    Dark,
    Light,
}

impl Player {
    fn other(self) -> Self {
        match self {
            Player::Dark => Player::Light,
            Player::Light => Player::Dark,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PieceKind {
    Man,
    King,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cell {
    pub row: u8,
    pub col: u8,
}

impl Cell {
    pub fn idx(self) -> usize {
        self.row as usize * 8 + self.col as usize
    }

    pub fn from_idx(i: usize) -> Option<Self> {
        if i >= 64 {
            return None;
        }
        Some(Cell {
            row: (i / 8) as u8,
            col: (i % 8) as u8,
        })
    }
}

/// At least two cells: start, then each landing square after each step or jump in the same turn.
/// Serializes as a plain JSON array of cells on the wire.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MovePath(pub Vec<Cell>);

impl Action<Checkers> for MovePath {
    type Error = String;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Piece {
    pub owner: Player,
    pub kind: PieceKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Board {
    /// `None` = empty; index `row * 8 + col`, length 64.
    cells: Vec<Option<Piece>>,
}

impl Board {
    fn empty() -> Self {
        Self {
            cells: vec![None; 64],
        }
    }

    fn get(&self, c: Cell) -> Option<Option<Piece>> {
        if c.row >= 8 || c.col >= 8 {
            return None;
        }
        if !is_dark(c) {
            return None;
        }
        Some(self.cells.get(c.idx()).copied().unwrap_or(None))
    }

    fn set(&mut self, c: Cell, p: Option<Piece>) {
        if c.row < 8 && c.col < 8 && is_dark(c) {
            if let Some(slot) = self.cells.get_mut(c.idx()) {
                *slot = p;
            }
        }
    }

    fn initial() -> Self {
        let mut b = Self::empty();
        for row in 0..2u8 {
            for col in 0..8u8 {
                let c = Cell { row, col };
                if is_dark(c) {
                    b.set(c, Some(Piece { owner: Player::Dark, kind: PieceKind::Man }));
                }
            }
        }
        for row in 6..8u8 {
            for col in 0..8u8 {
                let c = Cell { row, col };
                if is_dark(c) {
                    b.set(c, Some(Piece { owner: Player::Light, kind: PieceKind::Man }));
                }
            }
        }
        b
    }

    fn count_pieces(&self, player: Player) -> usize {
        self.cells
            .iter()
            .filter(|x| matches!(x, Some(p) if p.owner == player))
            .count()
    }

    fn clone_cells(&self) -> [Option<Piece>; 64] {
        let mut a = [None; 64];
        for i in 0..64 {
            a[i] = self.cells.get(i).copied().flatten();
        }
        a
    }

    /// Raw 8×8 linear cells (index `row * 8 + col`); `None` on light squares.
    pub fn cells(&self) -> &[Option<Piece>] {
        &self.cells
    }
}

fn is_dark(c: Cell) -> bool {
    (c.row + c.col) % 2 == 1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    pub config: Config,
    pub board: Board,
    pub current_player: Player,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerState {
    pub player: Player,
    pub state: State,
}

impl GamePlayerStateTrait<Checkers> for PlayerState {
    fn init(config: &<Checkers as GameCore>::Config, player: <Checkers as GameCore>::Player) -> Self {
        Self {
            player,
            state: Checkers::init(config),
        }
    }

    fn get_player(&self) -> <Checkers as GameCore>::Player {
        self.player
    }

    fn can_take_action(
        &self,
        action: &<Checkers as GameCore>::Action,
    ) -> Result<(), <<Checkers as GameCore>::Action as Action<Checkers>>::Error> {
        if self.state.current_player != self.player {
            return Err("Not your turn".into());
        }
        if action.0.len() < 2 {
            return Err("Move path needs at least start and end cell".into());
        }
        let path = MovePath(action.0.clone());
        if !legal_moves(&self.state, self.player).contains(&path) {
            return Err("Illegal move".into());
        }
        Ok(())
    }

    fn apply_event(&mut self, event: &<Checkers as GameCore>::PlayerEvent) {
        let PlayerEvent { player, path } = event;
        apply_turn_for_player(&mut self.state.board, path, *player);
        self.state.current_player = player.other();
    }
}

impl PlayerState {
    /// Client-side check before sending a move (turn + rules).
    pub fn validate_move_for_send(&self, path: &MovePath) -> Result<(), String> {
        GamePlayerStateTrait::can_take_action(self, path)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerEvent {
    pub player: Player,
    pub path: MovePath,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GameOutcome {
    Win(Player),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlayerOutcome {
    Win,
    Loss,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Config;

impl Default for Config {
    fn default() -> Self {
        Self
    }
}

impl<'de> Deserialize<'de> for Config {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Config;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("null or empty checkers config object")
            }
            fn visit_none<E>(self) -> Result<Config, E> {
                Ok(Config)
            }
            fn visit_unit<E>(self) -> Result<Config, E> {
                Ok(Config)
            }
            fn visit_map<M>(self, mut map: M) -> Result<Config, M::Error>
            where
                M: MapAccess<'de>,
            {
                while map.next_entry::<de::IgnoredAny, de::IgnoredAny>()?.is_some() {}
                Ok(Config)
            }
        }
        deserializer.deserialize_any(V)
    }
}

impl GameConfig<Checkers> for Config {
    type ValidationError = String;

    fn validate(&self) -> Result<(), Self::ValidationError> {
        Ok(())
    }

    fn get_players(&self) -> Vec<Player> {
        vec![Player::Dark, Player::Light]
    }
}

fn forward_dir(owner: Player) -> i8 {
    match owner {
        Player::Dark => 1,
        Player::Light => -1,
    }
}

/// Single-step deltas for men (non-capture): one step diagonally forward.
fn man_step_deltas(owner: Player) -> [(i8, i8); 2] {
    let f = forward_dir(owner);
    [(f, -1), (f, 1)]
}

/// All four diagonal directions for kings (unit steps for generation; longer slides built by repeating).
const KING_DIRS: [(i8, i8); 4] = [(-1, -1), (-1, 1), (1, -1), (1, 1)];

fn cell_offset(c: Cell, dr: i8, dc: i8) -> Option<Cell> {
    let r = i16::from(c.row) + i16::from(dr);
    let col = i16::from(c.col) + i16::from(dc);
    if r < 0 || r > 7 || col < 0 || col > 7 {
        return None;
    }
    Some(Cell {
        row: r as u8,
        col: col as u8,
    })
}

fn cells_between_exclusive(a: Cell, b: Cell) -> Option<Vec<Cell>> {
    let dr = b.row as i16 - a.row as i16;
    let dc = b.col as i16 - a.col as i16;
    if dr.abs() != dc.abs() || dr == 0 {
        return None;
    }
    let steps = dr.unsigned_abs() as i32;
    let sdr = dr.signum() as i8;
    let sdc = dc.signum() as i8;
    let mut out = Vec::new();
    for k in 1..steps {
        out.push(cell_offset(a, sdr * k as i8, sdc * k as i8)?);
    }
    Some(out)
}

/// True if every segment has only empty squares between endpoints (no jumps over pieces).
fn path_is_non_capture(board: &Board, path: &[Cell]) -> bool {
    for w in path.windows(2) {
        let Some(between) = cells_between_exclusive(w[0], w[1]) else {
            return false;
        };
        for c in between {
            if !matches!(board.get(c), Some(None)) {
                return false;
            }
        }
    }
    true
}

fn maybe_promote(mut p: Piece, at: Cell) -> Piece {
    if p.kind != PieceKind::Man {
        return p;
    }
    match p.owner {
        Player::Dark if at.row == 7 => p.kind = PieceKind::King,
        Player::Light if at.row == 0 => p.kind = PieceKind::King,
        _ => {}
    }
    p
}

/// Opponent pieces removed on one diagonal segment (empty for a slide).
fn remove_captures_on_segment(board: &mut Board, mover: Player, kind: PieceKind, from: Cell, to: Cell) {
    let Some(between) = cells_between_exclusive(from, to) else {
        return;
    };
    if between.is_empty() {
        return;
    }
    match kind {
        PieceKind::Man | PieceKind::King => {
            for c in between {
                if let Some(p) = board.get(c).flatten() {
                    if p.owner != mover {
                        board.set(c, None);
                    }
                }
            }
        }
    }
}

/// How many opponent pieces this path removes (one per capture segment).
fn path_capture_count(path: &[Cell]) -> usize {
    if path.len() < 2 {
        return 0;
    }
    (path.len() - 1) as usize
}

/// Apply a validated move path on the board (mutates `board` in place).
/// Apply a server-broadcast [`PlayerEvent`] to a copy of game state (client-side mirror).
pub fn apply_event_to_state(state: &mut State, event: &PlayerEvent) {
    apply_turn_for_player(&mut state.board, &event.path, event.player);
    state.current_player = event.player.other();
}

fn piece_has_any_capture(board: &Board, player: Player, from: Cell) -> bool {
    enumerate_all_capture_paths(board, player)
        .iter()
        .any(|p| p.0.first().copied() == Some(from))
}

fn max_captures_from_square(board: &Board, player: Player, from: Cell) -> usize {
    enumerate_all_capture_paths(board, player)
        .iter()
        .filter(|p| p.0.first().copied() == Some(from))
        .map(|p| path_capture_count(&p.0))
        .max()
        .unwrap_or(0)
}

/// One piece to remove when the player avoids capture by moving a piece that could not capture.
fn choose_sacrifice_capturer(board: &Board, player: Player) -> Option<Cell> {
    let paths = max_capture_paths(board, player);
    if paths.is_empty() {
        return None;
    }
    let mut candidates: Vec<Cell> = paths.iter().filter_map(|p| p.0.first().copied()).collect();
    candidates.sort_by_key(|c| (c.row, c.col));
    candidates.dedup();
    // Ascending sort: earlier = removed. Kings before men; then more captures from that square first.
    candidates.sort_by(|&a, &b| {
        let king_last = |c: Cell| -> u8 {
            match board.get(c).flatten().map(|p| p.kind) {
                Some(PieceKind::King) => 0u8,
                _ => 1u8,
            }
        };
        let ma = max_captures_from_square(board, player, a);
        let mb = max_captures_from_square(board, player, b);
        king_last(a)
            .cmp(&king_last(b))
            .then_with(|| mb.cmp(&ma))
            .then_with(|| a.row.cmp(&b.row))
            .then_with(|| a.col.cmp(&b.col))
    });
    candidates.first().copied()
}

/// Applies a full turn (path, promotions, optional sacrifice when captures were skipped).
fn apply_turn_for_player(board: &mut Board, path: &MovePath, player: Player) {
    let had_any_capture = any_capture_exists(board, player);
    let non_capture = path_is_non_capture(board, &path.0);
    let start = path.0.first().copied();
    let mover_could_capture =
        start.is_some_and(|s| piece_has_any_capture(board, player, s));
    let sacrifice_cell = if had_any_capture && non_capture && !mover_could_capture {
        choose_sacrifice_capturer(board, player)
    } else {
        None
    };

    apply_path_on_board(board, path);
    promote_men(board);

    if had_any_capture && non_capture {
        if mover_could_capture {
            if let Some(end) = path.0.last().copied() {
                board.set(end, None);
            }
        } else if let Some(sac) = sacrifice_cell {
            board.set(sac, None);
        }
    }
}

fn apply_path_on_board(board: &mut Board, path: &MovePath) {
    let cells = &path.0;
    if cells.len() < 2 {
        return;
    }
    let start = cells[0];
    let mut moving = board.cells[start.idx()]
        .take()
        .expect("validated path");
    let mut cur = start;
    for w in cells.windows(2) {
        let b = w[1];
        remove_captures_on_segment(board, moving.owner, moving.kind, cur, b);
        board.set(cur, None);
        moving = maybe_promote(moving, b);
        board.set(b, Some(moving));
        cur = b;
    }
}

fn promote_men(board: &mut Board) {
    for col in 0..8u8 {
        let c = Cell { row: 7, col };
        if let Some(Some(mut p)) = board.get(c) {
            if p.owner == Player::Dark && p.kind == PieceKind::Man {
                p.kind = PieceKind::King;
                board.set(c, Some(p));
            }
        }
        let c2 = Cell { row: 0, col };
        if let Some(Some(mut p)) = board.get(c2) {
            if p.owner == Player::Light && p.kind == PieceKind::Man {
                p.kind = PieceKind::King;
                board.set(c2, Some(p));
            }
        }
    }
}

/// Enumerate simple (non-capture) moves: men one step forward; kings slide along empty diagonals.
fn simple_moves(board: &Board, player: Player) -> Vec<MovePath> {
    let mut out = Vec::new();
    for row in 0..8u8 {
        for col in 0..8u8 {
            let from = Cell { row, col };
            if !is_dark(from) {
                continue;
            }
            let Some(Some(piece)) = board.get(from) else {
                continue;
            };
            if piece.owner != player {
                continue;
            }
            match piece.kind {
                PieceKind::Man => {
                    for (dr, dc) in man_step_deltas(player) {
                        if let Some(to) = cell_offset(from, dr, dc) {
                            if matches!(board.get(to), Some(None)) {
                                out.push(MovePath(vec![from, to]));
                            }
                        }
                    }
                }
                PieceKind::King => {
                    for (dr, dc) in KING_DIRS {
                        let mut k: i8 = 1;
                        while let Some(to) = cell_offset(from, dr * k, dc * k) {
                            if !matches!(board.get(to), Some(None)) {
                                break;
                            }
                            out.push(MovePath(vec![from, to]));
                            k = k.saturating_add(1);
                        }
                    }
                }
            }
        }
    }
    out
}

/// One jump leg from `from` with `piece` on `work` board; returns (landing, captured cell, new board scratch).
fn single_jumps_from(
    work: &[Option<Piece>; 64],
    from: Cell,
    piece: Piece,
) -> Vec<(Cell, Cell, [Option<Piece>; 64])> {
    let mut res = Vec::new();
    match piece.kind {
        PieceKind::Man => {
            for &(dr, dc) in &man_step_deltas(piece.owner) {
                let mid = match cell_offset(from, dr, dc) {
                    Some(m) => m,
                    None => continue,
                };
                let land = match cell_offset(from, dr * 2, dc * 2) {
                    Some(l) => l,
                    None => continue,
                };
                if !is_dark(mid) || !is_dark(land) {
                    continue;
                }
                let mid_i = mid.idx();
                let land_i = land.idx();
                if work[land_i].is_some() {
                    continue;
                }
                let Some(captured) = work[mid_i] else {
                    continue;
                };
                if captured.owner == piece.owner {
                    continue;
                }
                let mut next = *work;
                next[from.idx()] = None;
                next[mid_i] = None;
                let landed = maybe_promote(piece, land);
                next[land_i] = Some(landed);
                res.push((land, mid, next));
            }
        }
        PieceKind::King => {
            for &(dr, dc) in &KING_DIRS {
                let mut k: i8 = 1;
                let mut enemy_at: Option<Cell> = None;
                loop {
                    let Some(c) = cell_offset(from, dr * k, dc * k) else {
                        break;
                    };
                    if !is_dark(c) {
                        break;
                    }
                    let ci = c.idx();
                    match work[ci] {
                        None => {
                            if enemy_at.is_some() {
                                let land = c;
                                let cap = enemy_at.expect("enemy");
                                let cap_i = cap.idx();
                                let mut next = *work;
                                next[from.idx()] = None;
                                next[cap_i] = None;
                                let landed = maybe_promote(piece, land);
                                next[land.idx()] = Some(landed);
                                res.push((land, cap, next));
                            }
                            k = k.saturating_add(1);
                        }
                        Some(p) => {
                            if p.owner == piece.owner {
                                break;
                            }
                            if enemy_at.is_some() {
                                break;
                            }
                            enemy_at = Some(c);
                            k = k.saturating_add(1);
                        }
                    }
                }
            }
        }
    }
    res
}

/// DFS all terminal capture chains from `from` / `piece` on `work`; each terminal path includes full cell sequence.
fn dfs_capture_from(
    work: &[Option<Piece>; 64],
    from: Cell,
    piece: Piece,
    path: Vec<Cell>,
    out: &mut Vec<MovePath>,
) {
    let jumps = single_jumps_from(work, from, piece);
    if jumps.is_empty() {
        if path.len() >= 2 && path_capture_count(&path) > 0 {
            out.push(MovePath(path));
        }
        return;
    }
    for (land, _captured, next_board) in jumps {
        let at_land = next_board[land.idx()].expect("landed");
        let mut p = path.clone();
        p.push(land);
        dfs_capture_from(&next_board, land, at_land, p, out);
    }
}

fn enumerate_all_capture_paths(board: &Board, player: Player) -> Vec<MovePath> {
    let work = board.clone_cells();
    let mut all_terminal: Vec<MovePath> = Vec::new();

    for row in 0..8u8 {
        for col in 0..8u8 {
            let from = Cell { row, col };
            if !is_dark(from) {
                continue;
            }
            let Some(Some(piece)) = board.get(from) else {
                continue;
            };
            if piece.owner != player {
                continue;
            }
            dfs_capture_from(&work, from, piece, vec![from], &mut all_terminal);
        }
    }

    all_terminal.sort_by(|a, b| {
        a.0.iter()
            .map(|c| (c.row, c.col))
            .cmp(b.0.iter().map(|c| (c.row, c.col)))
    });
    all_terminal.dedup_by(|a, b| a.0 == b.0);
    all_terminal
}

/// True if this player has any capture sequence (any length).
fn any_capture_exists(board: &Board, player: Player) -> bool {
    !enumerate_all_capture_paths(board, player).is_empty()
}

/// Max-capture paths; if a king can lead such a path, drop man-led paths at the same count.
fn max_capture_paths(board: &Board, player: Player) -> Vec<MovePath> {
    let all_terminal = enumerate_all_capture_paths(board, player);
    let max_c = all_terminal
        .iter()
        .map(|p| path_capture_count(&p.0))
        .max()
        .unwrap_or(0);
    if max_c == 0 {
        return Vec::new();
    }
    let mut acc: Vec<MovePath> = all_terminal
        .into_iter()
        .filter(|p| path_capture_count(&p.0) == max_c)
        .collect();
    let king_can_max = acc.iter().any(|p| {
        board
            .get(p.0[0])
            .flatten()
            .is_some_and(|pc| pc.kind == PieceKind::King)
    });
    if king_can_max {
        acc.retain(|p| {
            board
                .get(p.0[0])
                .flatten()
                .is_some_and(|pc| pc.kind == PieceKind::King)
        });
    }
    acc
}

pub fn legal_moves(state: &State, player: Player) -> Vec<MovePath> {
    if player != state.current_player {
        return Vec::new();
    }
    let mut out = max_capture_paths(&state.board, player);
    out.extend(simple_moves(&state.board, player));
    out.sort_by(|a, b| {
        a.0.iter()
            .map(|c| (c.row, c.col))
            .cmp(b.0.iter().map(|c| (c.row, c.col)))
    });
    out.dedup_by(|a, b| a.0 == b.0);
    out
}

/// Cells that may be chosen as the next step while building a path: empty prefix means
/// legal move starts; otherwise extends a legal prefix (multi-jump chains).
pub fn legal_next_cells(state: &State, player: Player, prefix: &[Cell]) -> Vec<Cell> {
    let moves = legal_moves(state, player);
    let mut out: Vec<Cell> = Vec::new();
    for m in &moves {
        let cells = &m.0;
        if prefix.len() > cells.len() {
            continue;
        }
        if cells[..prefix.len()] != *prefix {
            continue;
        }
        if let Some(&c) = cells.get(prefix.len()) {
            out.push(c);
        }
    }
    out.sort_by_key(|c| (c.row, c.col));
    out.dedup();
    out
}

fn side_has_legal_moves(state: &State, player: Player) -> bool {
    let mut s = state.clone();
    s.current_player = player;
    !legal_moves(&s, player).is_empty()
}

fn check_game_over_state(state: &State) -> Option<GameOutcome> {
    let p = state.current_player;
    if state.board.count_pieces(p) == 0 {
        return Some(GameOutcome::Win(p.other()));
    }
    if state.board.count_pieces(p.other()) == 0 {
        return Some(GameOutcome::Win(p));
    }
    if !side_has_legal_moves(state, p) {
        return Some(GameOutcome::Win(p.other()));
    }
    None
}

impl GameCore for Checkers {
    type Config = Config;
    type State = State;
    type Action = MovePath;
    type Player = Player;
    type PlayerState = PlayerState;
    type Event = ();
    type PlayerEvent = PlayerEvent;
    type Result = GameOutcome;
    type PlayerResult = PlayerOutcome;

    fn init(config: &Self::Config) -> Self::State {
        State {
            config: config.clone(),
            board: Board::initial(),
            current_player: Player::Dark,
        }
    }

    fn take_action(
        state: &mut Self::State,
        player_action: game::PlayerAction<Self>,
    ) -> Vec<Self::Event> {
        let game::PlayerAction { player, action } = player_action;
        apply_turn_for_player(&mut state.board, &action, player);
        state.current_player = player.other();
        vec![]
    }

    fn check_game_over(state: &Self::State) -> Option<Self::Result> {
        check_game_over_state(state)
    }

    fn derive_player_event(
        _state: &Self::State,
        _player: &Self::Player,
        event: &game::InGameEvent<Self>,
    ) -> Option<Self::PlayerEvent> {
        match event {
            game::InGameEvent::PlayerAction(pa) => Some(PlayerEvent {
                player: pa.player,
                path: pa.action.clone(),
            }),
            _ => None,
        }
    }

    fn derive_player_result(
        _state: &Self::State,
        player: &Self::Player,
        result: &Self::Result,
    ) -> Self::PlayerResult {
        match result {
            GameOutcome::Win(w) if *w == *player => PlayerOutcome::Win,
            GameOutcome::Win(_) => PlayerOutcome::Loss,
        }
    }

    fn scores_at_end(result: &Self::Result) -> Vec<(Self::Player, f64)> {
        match result {
            GameOutcome::Win(w) => vec![
                (Player::Dark, if *w == Player::Dark { 1.0 } else { 0.0 }),
                (Player::Light, if *w == Player::Light { 1.0 } else { 0.0 }),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_dark_to_move() {
        let s = Checkers::init(&Config);
        assert_eq!(s.current_player, Player::Dark);
        assert!(s.board.count_pieces(Player::Dark) > 0);
    }

    #[test]
    fn simple_move_forward() {
        let mut fs = game::FullState {
            config: Config,
            state: Checkers::init(&Config),
            actions_made: vec![],
        };
        // Dark man on (2,3) -> (3,4) typical — pick a valid dark man one step
        let st = &mut fs.state;
        let m = legal_moves(st, Player::Dark);
        assert!(!m.is_empty(), "opening should have moves");
        let first = m[0].clone();
        let ps = PlayerState::init(&Config, Player::Dark);
        Checkers::apply_action(&mut fs, game::PlayerAction { player: Player::Dark, action: first }, &ps)
            .expect("legal");
    }

    #[test]
    fn light_has_legal_move_after_dark_turn() {
        let mut fs = game::FullState {
            config: Config,
            state: Checkers::init(&Config),
            actions_made: vec![],
        };
        let dark_ps = PlayerState::init(&Config, Player::Dark);
        let dark_move = legal_moves(&fs.state, Player::Dark)
            .into_iter()
            .next()
            .expect("dark opening move must exist");
        Checkers::apply_action(
            &mut fs,
            game::PlayerAction {
                player: Player::Dark,
                action: dark_move,
            },
            &dark_ps,
        )
        .expect("dark move legal");
        assert_eq!(fs.state.current_player, Player::Light);
        assert!(
            !legal_moves(&fs.state, Player::Light).is_empty(),
            "light must have legal replies"
        );
    }

    #[test]
    fn light_player_state_accepts_a_light_move_on_light_turn() {
        let mut fs = game::FullState {
            config: Config,
            state: Checkers::init(&Config),
            actions_made: vec![],
        };
        let dark_ps = PlayerState::init(&Config, Player::Dark);
        let dark_move = legal_moves(&fs.state, Player::Dark)
            .into_iter()
            .next()
            .expect("dark opening move must exist");
        Checkers::apply_action(
            &mut fs,
            game::PlayerAction {
                player: Player::Dark,
                action: dark_move,
            },
            &dark_ps,
        )
        .expect("dark move legal");

        let light_ps = PlayerState {
            player: Player::Light,
            state: fs.state.clone(),
        };
        let light_move = legal_moves(&fs.state, Player::Light)
            .into_iter()
            .next()
            .expect("light move should exist");
        assert!(
            light_ps.validate_move_for_send(&light_move).is_ok(),
            "light move should be accepted on light turn"
        );
    }

    #[test]
    fn light_can_apply_action_after_dark_move() {
        let mut fs = game::FullState {
            config: Config,
            state: Checkers::init(&Config),
            actions_made: vec![],
        };
        let dark_ps = PlayerState::init(&Config, Player::Dark);
        let dark_move = legal_moves(&fs.state, Player::Dark)
            .into_iter()
            .next()
            .expect("dark opening move must exist");
        Checkers::apply_action(
            &mut fs,
            game::PlayerAction {
                player: Player::Dark,
                action: dark_move,
            },
            &dark_ps,
        )
        .expect("dark move legal");

        let light_move = legal_moves(&fs.state, Player::Light)
            .into_iter()
            .next()
            .expect("light move should exist");
        let light_ps = PlayerState {
            player: Player::Light,
            state: fs.state.clone(),
        };
        Checkers::apply_action(
            &mut fs,
            game::PlayerAction {
                player: Player::Light,
                action: light_move,
            },
            &light_ps,
        )
        .expect("light move should apply");
        assert_eq!(fs.state.current_player, Player::Dark);
    }

    #[test]
    fn can_take_action_requires_current_player() {
        let s = Checkers::init(&Config);
        let dark_move = legal_moves(&s, Player::Dark)[0].clone();
        let light_ps = PlayerState::init(&Config, Player::Light);
        assert_eq!(
            light_ps.validate_move_for_send(&dark_move).unwrap_err(),
            "Not your turn"
        );
    }

    #[test]
    fn legal_next_cells_respects_turn() {
        let s = Checkers::init(&Config);
        assert!(!legal_next_cells(&s, Player::Dark, &[]).is_empty());
        assert!(legal_next_cells(&s, Player::Light, &[]).is_empty());
    }

    #[test]
    fn man_moves_use_row_as_forward_axis() {
        let s = Checkers::init(&Config);
        let m = legal_moves(&s, Player::Dark);
        assert!(!m.is_empty());
        for path in m {
            let a = path.0[0];
            let b = path.0[1];
            let dr = b.row as i16 - a.row as i16;
            let dc = (b.col as i16 - a.col as i16).unsigned_abs();
            assert_eq!(dr, 1, "dark opening man move must advance row by +1");
            assert_eq!(dc, 1, "dark opening man move must stay diagonal");
        }
    }

    #[test]
    fn player_outcome_json() {
        let win: game::PlayerEvent<Checkers> = game::PlayerEvent::GameOver(PlayerOutcome::Win);
        assert_eq!(serde_json::to_string(&win).unwrap(), r#"{"GameOver":"Win"}"#);
    }
}
