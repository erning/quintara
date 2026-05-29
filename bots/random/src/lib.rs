//! `quintara-bot-random`：在「已有棋子附近」的合法点中均匀随机落子。
//!
//! 候选 = `ctx.legal_moves` ∩「离任意棋子切比雪夫距离 ≤2」。这把数百个空点收窄到与战局
//! 相关的几十个,使随机对局更聚拢、更像样;无近邻棋子（空盘）时优先落天元,天元不可下
//! 才退回任意合法点。

use quintara_bot::{MoveSource, StopFlag};
use quintara_model::{Board, Move, Position, TurnContext};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// 近邻随机 bot。
pub struct RandomBot {
    rng: StdRng,
}

impl RandomBot {
    /// 从 OS 熵源播种——每次构造都不同。
    #[must_use]
    pub fn new() -> Self {
        Self {
            rng: StdRng::from_entropy(),
        }
    }

    /// 用固定 seed 构造，便于复现。
    #[must_use]
    pub fn from_seed(seed: u64) -> Self {
        Self {
            rng: StdRng::seed_from_u64(seed),
        }
    }
}

impl Default for RandomBot {
    fn default() -> Self {
        Self::new()
    }
}

impl MoveSource for RandomBot {
    fn next_move(&mut self, ctx: &TurnContext, _stop: &StopFlag) -> Move {
        // 候选 = 离棋子 ≤2 的合法点。conductor 保证有合法着法。
        let candidates: Vec<Move> = ctx
            .legal_moves
            .iter()
            .copied()
            .filter(|mv| near_stone(&ctx.board, mv.position()))
            .collect();
        if !candidates.is_empty() {
            return candidates[self.rng.gen_range(0..candidates.len())];
        }
        // 无近邻棋子（空盘）：优先天元，否则任意合法点。
        let center = Move::Place(Position::new(ctx.board.height() / 2, ctx.board.width() / 2));
        if ctx.legal_moves.contains(&center) {
            return center;
        }
        ctx.legal_moves[self.rng.gen_range(0..ctx.legal_moves.len())]
    }
}

/// `pos`（空点）周围切比雪夫距离 ≤2 内是否有任意棋子。
fn near_stone(board: &Board, pos: Position) -> bool {
    let (height, width) = (i32::from(board.height()), i32::from(board.width()));
    let (row0, col0) = (i32::from(pos.row), i32::from(pos.col));
    for d_row in -2..=2 {
        for d_col in -2..=2 {
            if d_row == 0 && d_col == 0 {
                continue; // pos 本身是空点
            }
            let (row, col) = (row0 + d_row, col0 + d_col);
            if row < 0 || col < 0 || row >= height || col >= width {
                continue;
            }
            let (Ok(row), Ok(col)) = (u8::try_from(row), u8::try_from(col)) else {
                continue;
            };
            if board.stone_at(Position::new(row, col)).is_some() {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use quintara_model::{Cell, Color, RuleSet};

    fn ctx_with(board: Board) -> TurnContext {
        let legal = board
            .empty_positions()
            .into_iter()
            .map(Move::Place)
            .collect();
        TurnContext {
            board,
            side_to_move: Color::White,
            move_history: Vec::new(),
            legal_moves: legal,
            rule_set: RuleSet::freestyle(),
            timeout_turn: None,
            time_left: None,
        }
    }

    #[test]
    fn picks_near_an_existing_stone() {
        let mut board = Board::square(15);
        board.set(Position::new(7, 7), Cell::Stone(Color::Black));
        let ctx = ctx_with(board);
        let mut bot = RandomBot::from_seed(42);
        for _ in 0..50 {
            let pos = bot.next_move(&ctx, &StopFlag::new()).position();
            let d_row = (i32::from(pos.row) - 7).abs();
            let d_col = (i32::from(pos.col) - 7).abs();
            assert!(
                d_row <= 2 && d_col <= 2,
                "picked {pos:?} far from the stone"
            );
            assert!(!(d_row == 0 && d_col == 0), "picked the occupied point");
        }
    }

    #[test]
    fn empty_board_plays_center() {
        // 无棋子：无近邻候选 → 落天元（15×15 中心 = (7,7)）。
        let ctx = ctx_with(Board::square(15));
        let mut bot = RandomBot::from_seed(7);
        let pos = bot.next_move(&ctx, &StopFlag::new()).position();
        assert_eq!(pos, Position::new(7, 7));
    }
}
