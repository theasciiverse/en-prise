//! The computer player: alpha-beta with iterative deepening, a capture
//! search at the leaves, material and simple square tables. Runs in its own
//! thread; it plays sound chess, not grandmaster chess.

use crate::game::{insufficient, legal_moves, value};
use crate::rng::Rng;
use cozy_chess::{Board, Color, GameStatus, Move, Piece, Square};
use std::time::{Duration, Instant};

const MATE: i32 = 100_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Strength {
    Easy,
    Normal,
    Hard,
}

impl Strength {
    pub fn name(self) -> &'static str {
        match self {
            Strength::Easy => "Easy",
            Strength::Normal => "Normal",
            Strength::Hard => "Hard",
        }
    }
    pub fn next(self) -> Strength {
        match self {
            Strength::Easy => Strength::Normal,
            Strength::Normal => Strength::Hard,
            Strength::Hard => Strength::Easy,
        }
    }
    /// (deepest search, time per move, random noise in centipawns)
    fn settings(self) -> (u32, Duration, i32) {
        match self {
            Strength::Easy => (2, Duration::from_millis(300), 80),
            Strength::Normal => (5, Duration::from_millis(1200), 12),
            Strength::Hard => (12, Duration::from_millis(3500), 0),
        }
    }
}

// Square tables from White's side, a8 first (as a board is read), so a
// White piece on square s uses entry (7 - rank) * 8 + file.
#[rustfmt::skip]
const PAWN: [i32; 64] = [
     0,  0,  0,  0,  0,  0,  0,  0,
    50, 50, 50, 50, 50, 50, 50, 50,
    12, 14, 20, 30, 30, 20, 14, 12,
     6,  6, 10, 25, 25, 10,  6,  6,
     0,  0,  0, 20, 20,  0,  0,  0,
     5, -5,-10,  0,  0,-10, -5,  5,
     5, 10, 10,-20,-20, 10, 10,  5,
     0,  0,  0,  0,  0,  0,  0,  0,
];
#[rustfmt::skip]
const KNIGHT: [i32; 64] = [
   -50,-40,-30,-30,-30,-30,-40,-50,
   -40,-20,  0,  0,  0,  0,-20,-40,
   -30,  0, 10, 15, 15, 10,  0,-30,
   -30,  5, 15, 20, 20, 15,  5,-30,
   -30,  0, 15, 20, 20, 15,  0,-30,
   -30,  5, 10, 15, 15, 10,  5,-30,
   -40,-20,  0,  5,  5,  0,-20,-40,
   -50,-40,-30,-30,-30,-30,-40,-50,
];
#[rustfmt::skip]
const BISHOP: [i32; 64] = [
   -20,-10,-10,-10,-10,-10,-10,-20,
   -10,  0,  0,  0,  0,  0,  0,-10,
   -10,  0,  5, 10, 10,  5,  0,-10,
   -10,  5,  5, 10, 10,  5,  5,-10,
   -10,  0, 10, 10, 10, 10,  0,-10,
   -10, 10, 10, 10, 10, 10, 10,-10,
   -10,  5,  0,  0,  0,  0,  5,-10,
   -20,-10,-10,-10,-10,-10,-10,-20,
];
#[rustfmt::skip]
const ROOK: [i32; 64] = [
     0,  0,  0,  0,  0,  0,  0,  0,
     5, 10, 10, 10, 10, 10, 10,  5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
     0,  0,  0,  5,  5,  0,  0,  0,
];
#[rustfmt::skip]
const QUEEN: [i32; 64] = [
   -20,-10,-10, -5, -5,-10,-10,-20,
   -10,  0,  0,  0,  0,  0,  0,-10,
   -10,  0,  5,  5,  5,  5,  0,-10,
    -5,  0,  5,  5,  5,  5,  0, -5,
     0,  0,  5,  5,  5,  5,  0, -5,
   -10,  5,  5,  5,  5,  5,  0,-10,
   -10,  0,  5,  0,  0,  0,  0,-10,
   -20,-10,-10, -5, -5,-10,-10,-20,
];
#[rustfmt::skip]
const KING_MID: [i32; 64] = [
   -30,-40,-40,-50,-50,-40,-40,-30,
   -30,-40,-40,-50,-50,-40,-40,-30,
   -30,-40,-40,-50,-50,-40,-40,-30,
   -30,-40,-40,-50,-50,-40,-40,-30,
   -20,-30,-30,-40,-40,-30,-30,-20,
   -10,-20,-20,-20,-20,-20,-20,-10,
    20, 20,  0,  0,  0,  0, 20, 20,
    20, 30, 10,  0,  0, 10, 30, 20,
];
#[rustfmt::skip]
const KING_END: [i32; 64] = [
   -50,-40,-30,-20,-20,-30,-40,-50,
   -30,-20,-10,  0,  0,-10,-20,-30,
   -30,-10, 20, 30, 30, 20,-10,-30,
   -30,-10, 30, 40, 40, 30,-10,-30,
   -30,-10, 30, 40, 40, 30,-10,-30,
   -30,-10, 20, 30, 30, 20,-10,-30,
   -30,-30,  0,  0,  0,  0,-30,-30,
   -50,-30,-30,-30,-30,-30,-30,-50,
];

fn table_index(sq: Square, c: Color) -> usize {
    let (f, r) = (sq.file() as usize, sq.rank() as usize);
    let r = if c == Color::White { r } else { 7 - r };
    (7 - r) * 8 + f
}

/// From the side to move's point of view.
pub fn eval(b: &Board) -> i32 {
    let queens = b.pieces(Piece::Queen).len();
    let minors_and_rooks = (b.pieces(Piece::Knight) | b.pieces(Piece::Bishop) | b.pieces(Piece::Rook)).len();
    let endgame = queens == 0 || (queens <= 2 && minors_and_rooks <= 2);
    let mut score = 0;
    for c in [Color::White, Color::Black] {
        let mut side = 0;
        for p in Piece::ALL {
            for sq in b.colored_pieces(c, p) {
                let i = table_index(sq, c);
                side += if p == Piece::King { 0 } else { value(p) }
                    + match p {
                        Piece::Pawn => PAWN[i],
                        Piece::Knight => KNIGHT[i],
                        Piece::Bishop => BISHOP[i],
                        Piece::Rook => ROOK[i],
                        Piece::Queen => QUEEN[i],
                        Piece::King => {
                            if endgame {
                                KING_END[i]
                            } else {
                                KING_MID[i]
                            }
                        }
                    };
            }
        }
        if b.colored_pieces(c, Piece::Bishop).len() >= 2 {
            side += 30;
        }
        score += if c == b.side_to_move() { side } else { -side };
    }
    score
}

fn is_capture(b: &Board, m: Move) -> bool {
    b.color_on(m.to) == Some(!b.side_to_move()) || (b.piece_on(m.from) == Some(Piece::Pawn) && m.from.file() != m.to.file())
}

fn order(b: &Board, moves: &mut [Move], best: Option<Move>, killers: &[Option<Move>; 2]) {
    moves.sort_by_cached_key(|&m| {
        if Some(m) == best {
            return -1_000_000;
        }
        let mut k = 0;
        if is_capture(b, m) {
            let victim = b.piece_on(m.to).map_or(100, value);
            let attacker = b.piece_on(m.from).map_or(0, value);
            k -= 100_000 + victim * 10 - attacker / 10;
        } else if killers.contains(&Some(m)) {
            k -= 50_000;
        }
        if m.promotion == Some(Piece::Queen) {
            k -= 90_000;
        }
        k
    });
}

struct Search {
    deadline: Instant,
    nodes: u64,
    stopped: bool,
    /// positions on the way here, for repetitions
    path: Vec<u64>,
    killers: Vec<[Option<Move>; 2]>,
}

impl Search {
    fn out_of_time(&mut self) -> bool {
        self.nodes += 1;
        if self.nodes % 512 == 0 && Instant::now() > self.deadline {
            self.stopped = true;
        }
        self.stopped
    }

    fn quiesce(&mut self, b: &Board, mut alpha: i32, beta: i32, depth: u32) -> i32 {
        let stand = eval(b);
        if stand >= beta || depth == 0 || self.out_of_time() {
            return stand;
        }
        alpha = alpha.max(stand);
        let mut caps: Vec<Move> = legal_moves(b).into_iter().filter(|&m| is_capture(b, m) || m.promotion == Some(Piece::Queen)).collect();
        order(b, &mut caps, None, &[None, None]);
        for m in caps {
            let mut c = b.clone();
            c.play_unchecked(m);
            let s = -self.quiesce(&c, -beta, -alpha, depth - 1);
            if s >= beta {
                return s;
            }
            alpha = alpha.max(s);
        }
        alpha
    }

    fn negamax(&mut self, b: &Board, depth: u32, mut alpha: i32, beta: i32, ply: usize) -> i32 {
        if self.out_of_time() {
            return 0;
        }
        match b.status() {
            GameStatus::Won => return -MATE + ply as i32,
            GameStatus::Drawn => return 0,
            GameStatus::Ongoing => {}
        }
        let h = b.hash();
        if self.path.iter().rev().take(b.halfmove_clock() as usize + 1).skip(1).any(|&p| p == h) || insufficient(b) {
            return 0;
        }
        let in_check = !b.checkers().is_empty();
        let depth = if in_check { depth + 1 } else { depth };
        if depth == 0 {
            return self.quiesce(b, alpha, beta, 6);
        }
        let mut moves = legal_moves(b);
        if self.killers.len() <= ply {
            self.killers.resize(ply + 1, [None, None]);
        }
        let killers = self.killers[ply];
        order(b, &mut moves, None, &killers);
        let mut best = -MATE * 2;
        for m in moves {
            let mut c = b.clone();
            c.play_unchecked(m);
            self.path.push(c.hash());
            let s = -self.negamax(&c, depth - 1, -beta, -alpha, ply + 1);
            self.path.pop();
            best = best.max(s);
            alpha = alpha.max(s);
            if alpha >= beta {
                if !is_capture(b, m) {
                    let k = &mut self.killers[ply];
                    if k[0] != Some(m) {
                        k[1] = k[0];
                        k[0] = Some(m);
                    }
                }
                break;
            }
        }
        best
    }
}

/// The computer's move, None when it has none. `history` is every position
/// of the game so far, for repetitions.
pub fn choose(b: &Board, history: &[u64], strength: Strength, rng: &mut Rng) -> Option<Move> {
    let (max_depth, time, noise) = strength.settings();
    let start = Instant::now();
    let mut path = history.to_vec();
    path.push(b.hash());
    let mut s = Search { deadline: start + time, nodes: 0, stopped: false, path, killers: Vec::new() };
    let mut moves = legal_moves(b);
    if moves.is_empty() {
        return None;
    }
    if moves.len() == 1 {
        return Some(moves[0]);
    }
    let mut best: Option<(Move, i32)> = None;
    for depth in 1..=max_depth {
        order(b, &mut moves, best.map(|(m, _)| m), &[None, None]);
        let mut alpha = -MATE * 2;
        let mut this: Option<(Move, i32)> = None;
        for &m in &moves {
            let mut c = b.clone();
            c.play_unchecked(m);
            s.path.push(c.hash());
            let jitter = if noise > 0 { (rng.next() % (2 * noise as u64 + 1)) as i32 - noise } else { 0 };
            let score = -s.negamax(&c, depth - 1, -MATE * 2, -alpha + noise, 1) + jitter;
            s.path.pop();
            if s.stopped {
                break;
            }
            if this.is_none_or(|(_, v)| score > v) {
                this = Some((m, score));
                alpha = alpha.max(score);
            }
        }
        if s.stopped {
            // a depth cut short only counts when nothing deeper finished
            if best.is_none() {
                best = this;
            }
            break;
        }
        best = this;
        log::debug!("depth {depth}: {:?} after {} nodes, {:?}", best.map(|(m, v)| (m.to_string(), v)), s.nodes, start.elapsed());
        if best.is_some_and(|(_, v)| v.abs() > MATE - 100) {
            break;
        }
    }
    log::info!("computer ({}) chose {:?} after {} nodes in {:?}", strength.name(), best.map(|(m, v)| (m.to_string(), v)), s.nodes, start.elapsed());
    best.map(|(m, _)| m)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Game, Status, describe};

    #[test]
    fn mates_in_one() {
        // back-rank mate: Re8#
        let b: Board = "6k1/5ppp/8/8/8/8/8/4R1K1 w - - 0 1".parse().unwrap();
        let m = choose(&b, &[], Strength::Normal, &mut Rng::new(1)).unwrap();
        assert_eq!(describe(&b, m).san, "Re8#");
    }

    #[test]
    fn takes_a_free_queen() {
        let b: Board = "4k3/8/2q5/8/3N4/8/8/4K3 w - - 0 1".parse().unwrap();
        let m = choose(&b, &[], Strength::Normal, &mut Rng::new(1)).unwrap();
        assert_eq!(describe(&b, m).san, "Nxc6");
    }

    #[test]
    fn plays_a_game_against_itself() {
        let mut g = Game::new();
        let mut rng = Rng::new(3);
        for _ in 0..40 {
            if g.status() != Status::Playing {
                break;
            }
            let m = choose(&g.board, &g.hashes(), Strength::Easy, &mut rng).expect("a move");
            g.play(m);
        }
    }
}
