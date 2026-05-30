//! `quintara-bot-sage`：1-ply 棋型启发式。比 greedy 强:分活 / 死、认冲四活三、靠跨轴累加
//! 抓双威胁(叉)、按 `ctx.rule_set` 正确判定胜负。**不做搜索**(α-β / 威胁搜索是后续版本)。
//!
//! 每手:
//! 1. 候选 = `legal_moves` ∩ 离任意子 ≤2（空盘 → 天元）；
//! 2. 能赢就赢(`is_win_for` 按规则)；
//! 3. 否则必堵对手的成五点；
//! 4. 否则按 `我的棋型分 + W·对手棋型分` 取最高,**同分随机**。

mod eval;

use quintara_bot::{MoveSource, StopFlag};
use quintara_model::{Move, Position, TurnContext};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

// 同分随机即可,不需要可复现 seed:正式与测试都用系统熵。

/// 防守权重的分子 / 分母:进攻 ×10、防守 ×8（即 W≈0.8，略偏进攻）。
const OFFENSE_WEIGHT: i64 = 10;
const DEFENSE_WEIGHT: i64 = 8;

/// 棋型启发 bot。
pub struct SageBot {
    rng: StdRng,
}

impl SageBot {
    /// 从 OS 熵源播种——每次构造都不同（同分随机）。
    #[must_use]
    pub fn new() -> Self {
        Self {
            rng: StdRng::from_entropy(),
        }
    }

    /// 从若干等价点里随机取一个（调用方保证非空）。
    fn pick(&mut self, points: &[Position]) -> Position {
        points[self.rng.gen_range(0..points.len())]
    }
}

impl Default for SageBot {
    fn default() -> Self {
        Self::new()
    }
}

impl MoveSource for SageBot {
    fn next_move(&mut self, ctx: &TurnContext, _stop: &StopFlag) -> Move {
        let me = ctx.side_to_move;
        let opponent = me.opposite();
        let rule_set = ctx.rule_set;
        let board = &ctx.board;

        // 候选 = 离任意子 ≤2 的合法点。
        let candidates: Vec<Position> = ctx
            .legal_moves
            .iter()
            .map(|m| m.position())
            .filter(|&pos| eval::near_stone(board, pos))
            .collect();

        // 空盘（无近邻候选）：天元，否则任意合法点。
        if candidates.is_empty() {
            let center = Position::new(board.height() / 2, board.width() / 2);
            if ctx.legal_moves.iter().any(|m| m.position() == center) {
                return Move::Place(center);
            }
            return ctx.legal_moves[0];
        }

        // 能赢就赢。
        let wins: Vec<Position> = candidates
            .iter()
            .copied()
            .filter(|&pos| eval::wins_at(board, pos, me, rule_set))
            .collect();
        if !wins.is_empty() {
            return Move::Place(self.pick(&wins));
        }

        // 必堵对手成五点。
        let blocks: Vec<Position> = candidates
            .iter()
            .copied()
            .filter(|&pos| eval::wins_at(board, pos, opponent, rule_set))
            .collect();
        if !blocks.is_empty() {
            return Move::Place(self.pick(&blocks));
        }

        // 棋型分:进攻 + 防守；同分收集后随机。
        let mut best_score = i64::MIN;
        let mut best: Vec<Position> = Vec::new();
        for &pos in &candidates {
            let offense = eval::shape_score(board, pos, me);
            let defense = eval::shape_score(board, pos, opponent);
            let score = offense * OFFENSE_WEIGHT + defense * DEFENSE_WEIGHT;
            if score > best_score {
                best_score = score;
                best.clear();
                best.push(pos);
            } else if score == best_score {
                best.push(pos);
            }
        }
        Move::Place(self.pick(&best))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quintara_model::{Board, Cell, Color, RuleSet};

    fn ctx(board: Board, side: Color) -> TurnContext {
        let legal = board
            .empty_positions()
            .into_iter()
            .map(Move::Place)
            .collect();
        TurnContext {
            board,
            side_to_move: side,
            move_history: Vec::new(),
            legal_moves: legal,
            rule_set: RuleSet::freestyle(),
            timeout_turn: None,
            time_left: None,
        }
    }

    fn board_with(black: &[(u8, u8)], white: &[(u8, u8)]) -> Board {
        let mut board = Board::square(15);
        for &(r, c) in black {
            board.set(Position::new(r, c), Cell::Stone(Color::Black));
        }
        for &(r, c) in white {
            board.set(Position::new(r, c), Cell::Stone(Color::White));
        }
        board
    }

    #[test]
    fn takes_the_immediate_win() {
        // 黑方四连，落两端任一成五。
        let board = board_with(
            &[(7, 7), (7, 8), (7, 9), (7, 10)],
            &[(0, 0), (0, 1), (0, 2)],
        );
        let context = ctx(board.clone(), Color::Black);
        let mv = SageBot::new().next_move(&context, &StopFlag::new());
        assert!(
            eval::wins_at(&board, mv.position(), Color::Black, RuleSet::freestyle()),
            "should win immediately, played {:?}",
            mv.position()
        );
    }

    #[test]
    fn blocks_opponent_immediate_win() {
        // 白方四连 (7,7..10)，黑方无威胁 → 必堵 (7,6) 或 (7,11)。
        let board = board_with(&[(0, 0), (0, 1)], &[(7, 7), (7, 8), (7, 9), (7, 10)]);
        let context = ctx(board, Color::Black);
        let mv = SageBot::new()
            .next_move(&context, &StopFlag::new())
            .position();
        assert!(
            mv == Position::new(7, 6) || mv == Position::new(7, 11),
            "should block the four, played {mv:?}"
        );
    }

    #[test]
    fn winning_beats_blocking() {
        // 黑方可成五(7,7..10)，白方也四连(0,0..3)；应抢自己的胜,而非堵。
        let board = board_with(
            &[(7, 7), (7, 8), (7, 9), (7, 10)],
            &[(0, 0), (0, 1), (0, 2), (0, 3)],
        );
        let context = ctx(board.clone(), Color::Black);
        let mv = SageBot::new().next_move(&context, &StopFlag::new());
        assert!(
            eval::wins_at(&board, mv.position(), Color::Black, RuleSet::freestyle()),
            "should take its own win, played {:?}",
            mv.position()
        );
    }
}
