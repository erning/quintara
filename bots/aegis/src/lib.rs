//! `quintara-bot-aegis`:**骨架**。架构待定——刻意与 titan(bitboard + α-β + VCF)不同。
//!
//! 现在 [`AegisBot::next_move`] 只做三件**任何架构都需要**的前置:空盘走天元、能成五就成五、
//! 必堵对手成五点;其余一律交给 [`AegisBot::choose`]——**那里就是你的搜索架构插入点**。
//!
//! 当前 `choose` 是个占位实现(离子 ≤2 的候选里按 greedy「最长连子」攻防加权选,同分随机),
//! 只为让骨架能编译、能合法对弈。换架构(MCTS / PNS / 学习型评估 …)时**只改 `choose`**;
//! 需要更复杂的盘面表示 / 评估就在本 crate 里加模块(`board.rs` / `eval.rs` / `search.rs` …)。

use quintara_bot::{MoveSource, StopFlag};
use quintara_model::{Board, Cell, Color, Move, Position, TurnContext};
use quintara_rules::{is_win_for, longest_run_if_placed, RuleSet};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// 候选邻域半径:空点在此切比雪夫距离内有子才纳入候选(与 sage / titan 一致)。
const NEAR: i32 = 2;

/// Aegis bot(骨架)。
pub struct AegisBot {
    rng: StdRng,
}

impl AegisBot {
    #[must_use]
    pub fn new() -> Self {
        Self {
            rng: StdRng::from_entropy(),
        }
    }

    /// 从若干等价点里随机取一个(调用方保证非空)。
    fn pick(&mut self, points: &[Position]) -> Position {
        points[self.rng.gen_range(0..points.len())]
    }

    /// 候选 = 离任意子 ≤[`NEAR`] 的合法空点。
    fn candidates(ctx: &TurnContext) -> Vec<Position> {
        ctx.legal_moves
            .iter()
            .map(|m| m.position())
            .filter(|&p| near_stone(&ctx.board, p))
            .collect()
    }

    /// **架构插入点**:在 `candidates`(非空)里选一手。
    ///
    /// 占位实现 = greedy:每个候选按「我落此处最长连子 ×2 + 对手落此处最长连子」打分,取最高、同分随机。
    /// 换成你的搜索时只改这个方法,签名(拿到 `ctx` 与候选)保持即可。
    fn choose(&mut self, ctx: &TurnContext, candidates: &[Position]) -> Position {
        let me = ctx.side_to_move;
        let opp = me.opposite();
        let mut best_score = 0;
        let mut best = vec![candidates[0]];
        for &p in candidates {
            let score = longest_run_if_placed(&ctx.board, p, me) * 2
                + longest_run_if_placed(&ctx.board, p, opp);
            if score > best_score {
                best_score = score;
                best.clear();
                best.push(p);
            } else if score == best_score {
                best.push(p);
            }
        }
        self.pick(&best)
    }
}

impl Default for AegisBot {
    fn default() -> Self {
        Self::new()
    }
}

impl MoveSource for AegisBot {
    fn next_move(&mut self, ctx: &TurnContext, _stop: &StopFlag) -> Move {
        let me = ctx.side_to_move;
        let opp = me.opposite();
        let rule = ctx.rule_set;

        let candidates = Self::candidates(ctx);

        // 空盘(无近邻候选):天元,否则任意合法手。
        if candidates.is_empty() {
            let center = Position::new(ctx.board.height() / 2, ctx.board.width() / 2);
            if ctx.legal_moves.iter().any(|m| m.position() == center) {
                return Move::Place(center);
            }
            return ctx.legal_moves[0];
        }

        // 能赢就赢。
        if let Some(p) = candidates
            .iter()
            .copied()
            .find(|&p| wins_at(&ctx.board, p, me, rule))
        {
            return Move::Place(p);
        }

        // 必堵对手成五点(可能多点,随机取一)。
        let blocks: Vec<Position> = candidates
            .iter()
            .copied()
            .filter(|&p| wins_at(&ctx.board, p, opp, rule))
            .collect();
        if !blocks.is_empty() {
            return Move::Place(self.pick(&blocks));
        }

        // 其余 → 架构插入点。
        Move::Place(self.choose(ctx, &candidates))
    }
}

/// `color` 落子 `pos` 是否立即获胜(按规则正确判定)。
fn wins_at(board: &Board, pos: Position, color: Color, rule: RuleSet) -> bool {
    let mut board = board.clone();
    board.set(pos, Cell::Stone(color));
    is_win_for(&board, pos, rule, color)
}

/// `pos` 切比雪夫距离 ≤[`NEAR`] 内是否有任意一方棋子。
fn near_stone(board: &Board, pos: Position) -> bool {
    let (height, width) = (i32::from(board.height()), i32::from(board.width()));
    let (row0, col0) = (i32::from(pos.row), i32::from(pos.col));
    for d_row in -NEAR..=NEAR {
        for d_col in -NEAR..=NEAR {
            if d_row == 0 && d_col == 0 {
                continue;
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
