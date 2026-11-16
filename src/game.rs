//! The game of chess itself, on cozy-chess's move generator: the moves
//! played, take-backs, how a game ends, notation, and which pieces stand
//! en prise.

use cozy_chess::util::display_san_move;
use cozy_chess::{BitBoard, Board, Color, GameStatus, Move, Piece, Rank, Square, File, get_bishop_moves, get_king_moves, get_knight_moves, get_pawn_attacks, get_rook_moves};

/// What a move did, worked out before it is played, for the animation.
#[derive(Clone, Debug)]
pub struct Played {
    pub before: Board,
    pub piece: Piece,
    pub colour: Color,
    pub from: Square,
    /// where the piece ends up (for castling the king's square, not the rook's)
    pub to: Square,
    /// the piece taken and where it stood (en passant: not on `to`)
    pub captured: Option<(Piece, Square)>,
    /// castling: the rook's walk
    pub rook: Option<(Square, Square)>,
    pub promotion: Option<Piece>,
    pub san: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Playing,
    /// the side that gave mate
    Mate(Color),
    Stalemate,
    Fifty,
    Repetition,
    Material,
}

impl Status {
    pub fn over(self) -> bool {
        self != Status::Playing
    }
    pub fn describe(self) -> String {
        match self {
            Status::Playing => "playing".into(),
            Status::Mate(c) => format!("Checkmate. {} wins.", side(c)),
            Status::Stalemate => "Stalemate: a draw.".into(),
            Status::Fifty => "Fifty moves without a capture or a pawn move: a draw.".into(),
            Status::Repetition => "The same position three times: a draw.".into(),
            Status::Material => "Neither side can mate: a draw.".into(),
        }
    }
}

pub fn side(c: Color) -> &'static str {
    match c {
        Color::White => "White",
        Color::Black => "Black",
    }
}

pub fn value(p: Piece) -> i32 {
    match p {
        Piece::Pawn => 100,
        Piece::Knight => 320,
        Piece::Bishop => 330,
        Piece::Rook => 500,
        Piece::Queen => 900,
        Piece::King => 20_000,
    }
}

pub fn glyph(p: Piece, c: Color) -> char {
    let i = match p {
        Piece::King => 0,
        Piece::Queen => 1,
        Piece::Rook => 2,
        Piece::Bishop => 3,
        Piece::Knight => 4,
        Piece::Pawn => 5,
    };
    if c == Color::White { ['♔', '♕', '♖', '♗', '♘', '♙'][i] } else { ['♚', '♛', '♜', '♝', '♞', '♟'][i] }
}

pub fn name(p: Piece) -> &'static str {
    match p {
        Piece::Pawn => "pawn",
        Piece::Knight => "knight",
        Piece::Bishop => "bishop",
        Piece::Rook => "rook",
        Piece::Queen => "queen",
        Piece::King => "king",
    }
}

/// Every piece of colour `by` that attacks `sq`.
pub fn attackers(b: &Board, sq: Square, by: Color) -> BitBoard {
    let occ = b.occupied();
    let queens = b.pieces(Piece::Queen);
    ((get_knight_moves(sq) & b.pieces(Piece::Knight))
        | (get_king_moves(sq) & b.pieces(Piece::King))
        | (get_pawn_attacks(sq, !by) & b.pieces(Piece::Pawn))
        | (get_rook_moves(sq, occ) & (b.pieces(Piece::Rook) | queens))
        | (get_bishop_moves(sq, occ) & (b.pieces(Piece::Bishop) | queens)))
        & b.colors(by)
}

/// The pieces of `c` (kings aside) that are en prise: attacked and either
/// not defended or attacked by something worth less.
pub fn en_prise(b: &Board, c: Color) -> BitBoard {
    let mut out = BitBoard::EMPTY;
    for sq in b.colors(c) & !b.pieces(Piece::King) {
        let Some(p) = b.piece_on(sq) else { continue };
        let att = attackers(b, sq, !c);
        if att.is_empty() {
            continue;
        }
        let cheapest = att.into_iter().filter_map(|a| b.piece_on(a)).map(value).min().unwrap_or(0);
        if attackers(b, sq, c).is_empty() || cheapest < value(p) {
            out |= sq.bitboard();
        }
    }
    out
}

pub fn legal_moves(b: &Board) -> Vec<Move> {
    let mut v = Vec::with_capacity(48);
    b.generate_moves(|ms| {
        v.extend(ms);
        false
    });
    v
}

/// Castling in cozy-chess is the king taking its own rook.
fn castle(b: &Board, mv: Move) -> Option<(Square, Square, Square)> {
    let us = b.side_to_move();
    if b.piece_on(mv.from) != Some(Piece::King) || b.color_on(mv.to) != Some(us) {
        return None;
    }
    let rank = Rank::First.relative_to(us);
    let short = mv.to.file() > mv.from.file();
    let (king_file, rook_file) = if short { (File::G, File::F) } else { (File::C, File::D) };
    Some((Square::new(king_file, rank), mv.to, Square::new(rook_file, rank)))
}

/// The move a click from `from` to `to` means, as cozy-chess spells it
/// (castling by clicking the king's destination or the rook).
pub fn find_move(b: &Board, from: Square, to: Square, promotion: Option<Piece>) -> Option<Move> {
    legal_moves(b).into_iter().find(|m| m.from == from && (m.to == to || castle(b, *m).is_some_and(|(k, _, _)| k == to)) && m.promotion == promotion)
}

/// Where a legal move's piece lands, for showing targets.
pub fn landing(b: &Board, m: Move) -> Square {
    castle(b, m).map_or(m.to, |(k, _, _)| k)
}

pub fn describe(b: &Board, mv: Move) -> Played {
    let piece = b.piece_on(mv.from).expect("a piece to move");
    let colour = b.side_to_move();
    let mut to = mv.to;
    let mut rook = None;
    let mut captured = None;
    if let Some((k, r_from, r_to)) = castle(b, mv) {
        to = k;
        rook = Some((r_from, r_to));
    } else if let Some(p) = b.piece_on(mv.to) {
        captured = Some((p, mv.to));
    } else if piece == Piece::Pawn && mv.from.file() != mv.to.file() {
        let sq = Square::new(mv.to.file(), mv.from.rank());
        captured = Some((Piece::Pawn, sq));
    }
    Played { before: b.clone(), piece, colour, from: mv.from, to, captured, rook, promotion: mv.promotion, san: display_san_move(b, mv).to_string() }
}

pub struct Game {
    pub board: Board,
    /// every move so far, with the position before it
    pub played: Vec<Played>,
}

impl Game {
    pub fn new() -> Game {
        Game { board: Board::default(), played: Vec::new() }
    }

    pub fn play(&mut self, mv: Move) -> Played {
        let p = describe(&self.board, mv);
        self.board.play_unchecked(mv);
        self.played.push(p.clone());
        p
    }

    pub fn undo(&mut self) -> bool {
        match self.played.pop() {
            Some(p) => {
                self.board = p.before;
                true
            }
            None => false,
        }
    }

    pub fn hashes(&self) -> Vec<u64> {
        self.played.iter().map(|p| p.before.hash()).collect()
    }

    pub fn status(&self) -> Status {
        match self.board.status() {
            GameStatus::Won => return Status::Mate(!self.board.side_to_move()),
            GameStatus::Drawn => return if self.board.checkers().is_empty() && self.board.halfmove_clock() < 100 { Status::Stalemate } else { Status::Fifty },
            GameStatus::Ongoing => {}
        }
        let now = self.board.hash();
        if self.played.iter().filter(|p| p.before.hash() == now).count() >= 2 {
            return Status::Repetition;
        }
        if insufficient(&self.board) {
            return Status::Material;
        }
        Status::Playing
    }

    pub fn in_check(&self) -> bool {
        !self.board.checkers().is_empty()
    }

    /// The moves as "1. e4 e5" lines.
    pub fn lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        let mut i = 0;
        // a game set up with Black to move would start "1. ..."; ours never is
        while i < self.played.len() {
            let n = self.played[i].before.fullmove_number();
            let w = &self.played[i].san;
            let b = self.played.get(i + 1).map_or("", |p| p.san.as_str());
            out.push(format!("{n:>3}. {w:<8}{b}"));
            i += 2;
        }
        out
    }

    /// Pieces each side has taken, as (taken by White, taken by Black).
    pub fn taken(&self) -> (Vec<Piece>, Vec<Piece>) {
        let (mut w, mut b) = (Vec::new(), Vec::new());
        for p in &self.played {
            if let Some((k, _)) = p.captured {
                if p.colour == Color::White { w.push(k) } else { b.push(k) }
            }
        }
        w.sort_by_key(|&k| -value(k));
        b.sort_by_key(|&k| -value(k));
        (w, b)
    }
}

/// Bare kings, or a king and one minor piece against a bare king, or bishops
/// all on one colour.
pub fn insufficient(b: &Board) -> bool {
    let heavy = b.pieces(Piece::Pawn) | b.pieces(Piece::Rook) | b.pieces(Piece::Queen);
    if !heavy.is_empty() {
        return false;
    }
    let minors = b.pieces(Piece::Knight) | b.pieces(Piece::Bishop);
    if minors.len() <= 1 {
        return true;
    }
    if b.pieces(Piece::Knight).is_empty() {
        let dark = b.pieces(Piece::Bishop).into_iter().filter(|s| (s.file() as usize + s.rank() as usize) % 2 == 0).count();
        return dark == 0 || dark == b.pieces(Piece::Bishop).len() as usize;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sq(s: &str) -> Square {
        s.parse().unwrap()
    }

    #[test]
    fn castling_is_described_as_the_king_and_rook_walking() {
        let b: Board = "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1".parse().unwrap();
        let m = find_move(&b, sq("e1"), sq("g1"), None).expect("O-O");
        let p = describe(&b, m);
        assert_eq!(p.to, sq("g1"));
        assert_eq!(p.rook, Some((sq("h1"), sq("f1"))));
        assert_eq!(p.san, "O-O");
        assert!(p.captured.is_none());
        let m = find_move(&b, sq("e1"), sq("c1"), None).expect("O-O-O");
        assert_eq!(describe(&b, m).rook, Some((sq("a1"), sq("d1"))));
    }

    #[test]
    fn en_passant_takes_the_pawn_beside() {
        let b: Board = "4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 1".parse().unwrap();
        let m = find_move(&b, sq("e5"), sq("d6"), None).unwrap();
        assert_eq!(describe(&b, m).captured, Some((Piece::Pawn, sq("d5"))));
    }

    #[test]
    fn fools_mate_is_mate() {
        let mut g = Game::new();
        for (a, b) in [("f2", "f3"), ("e7", "e5"), ("g2", "g4"), ("d8", "h4")] {
            let m = find_move(&g.board, sq(a), sq(b), None).unwrap();
            g.play(m);
        }
        assert_eq!(g.status(), Status::Mate(Color::Black));
        assert_eq!(g.played.last().unwrap().san, "Qh4#");
    }

    #[test]
    fn repetition_draws() {
        let mut g = Game::new();
        for _ in 0..2 {
            for (a, b) in [("g1", "f3"), ("g8", "f6"), ("f3", "g1"), ("f6", "g8")] {
                let m = find_move(&g.board, sq(a), sq(b), None).unwrap();
                g.play(m);
            }
        }
        assert_eq!(g.status(), Status::Repetition);
        assert!(g.undo());
        assert_eq!(g.status(), Status::Playing);
    }

    #[test]
    fn a_hanging_piece_is_en_prise() {
        // the black knight on c6 is attacked by a pawn: en prise though defended
        let b: Board = "r1bqkbnr/pppppppp/2n5/1P6/8/8/P1PPPPPP/RNBQKBNR b KQkq - 0 1".parse().unwrap();
        assert!(en_prise(&b, Color::Black).has(sq("c6")));
        assert!(!en_prise(&b, Color::White).has(sq("b5")) || attackers(&b, sq("b5"), Color::Black).len() > 0);
    }
}
