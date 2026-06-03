//! `quintara-bot-onyx`：面向 **freestyle 15×15 执黑必胜** 的攻击型 Gomoku 引擎。
//!
//! 每手决策的优先级（attack-first）：
//! 1. 能立即成五 → 取胜；
//! 2. 对手有立即成五点 → 必堵（多堵点选最有进攻价值者）；
//! 3. 自己存在 **VCF**（连续四）强制胜 → 走杀；
//! 4. 防守过滤：剔除走完后让对手获得立即胜 / VCF 杀的着；
//! 5. 在安全着集合上做时间预算内的迭代加深 α-β，取最优。
//!
//! 规则差异只经 `ctx.rule_set` 投影成 [`Win`]；Onyx 主攻 freestyle，不为 standard / renju 优化。

// NOTE: 引擎内部统一用 i32 棋盘坐标做索引与连子算术，坐标恒在 0..=board_size（≤32）的小范围，
// 与 usize / u8 之间的转换不会截断或符号溢出；为热路径可读性，在 crate 级豁免这三类 cast 检查。
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]

mod eval;
mod grid;
mod search;
mod vct;

use std::time::{Duration, Instant};

use quintara_bot::{MoveSource, StopFlag};
use quintara_model::{Move, Position, TurnContext};

use grid::{code_of, Grid, Win};

/// 每手默认思考预算（协议与命令行都未给出时）。
const DEFAULT_BUDGET: Duration = Duration::from_secs(1);
/// 固定安全余量（ms）：给协议 stdio 往返 + 搜索单节点溢出预留。搜索各阶段已严守自身 deadline，
/// 故只需小余量;arbiter 的 timeout tolerance 才是真正的安全网，无需再按比例自废思考时间。
const SAFETY_MARGIN_MS: u64 = 40;
/// 防守过滤占总预算的比例上限。
const DEFENSE_NUM: u32 = 55;
/// VCF 进攻搜索占总预算的比例上限。
const VCF_NUM: u32 = 35;
/// VCT 进攻搜索占总预算的比例上限（在 VCF 之后尝试）。很小：VCT 只为快速逮到强制胜，
/// 多数局面找不到，须把时间留给做防守的 α-β。
const VCT_NUM: u32 = 12;
/// VCT 根搜索的节点上限（很紧，避免非胜局面空耗）。
const VCT_NODE_CAP: u64 = 12_000;
/// 防守时单个候选的 VCF 否证时间上限。
const PER_DEFENSE_CHECK: Duration = Duration::from_millis(15);
/// 进入 α-β 的根候选数上限。
const MAX_ROOTS: usize = 24;
/// α-β 迭代加深的层数上限（仍以时间为先约束）。
const MAX_DEPTH: i32 = 32;

/// Onyx 攻击型引擎。
pub struct OnyxBot {
    /// 命令行覆写的每手预算；`None` 时取协议 `timeout_turn`，再退默认。
    budget: Option<Duration>,
}

impl OnyxBot {
    #[must_use]
    pub fn new() -> Self {
        Self { budget: None }
    }

    /// 设定每手思考预算（仍受协议 `INFO timeout_turn` 约束）。
    #[must_use]
    pub fn with_budget(mut self, budget: Duration) -> Self {
        self.budget = Some(budget);
        self
    }

    /// 本手的有效计算时长：min(命令行, 协议 `timeout_turn`) 扣掉固定 stdio 安全余量。
    fn effective_budget(&self, ctx: &TurnContext) -> Duration {
        let base = match (self.budget, ctx.timeout_turn) {
            (Some(a), Some(b)) => a.min(b),
            (Some(a), None) => a,
            (None, Some(b)) => b,
            (None, None) => DEFAULT_BUDGET,
        };
        let ms = u64::try_from(base.as_millis()).unwrap_or(u64::MAX);
        // 只扣固定 stdio 余量；下限 20ms 防止极小预算下卡死。
        Duration::from_millis(ms.saturating_sub(SAFETY_MARGIN_MS).max(20))
    }
}

impl Default for OnyxBot {
    fn default() -> Self {
        Self::new()
    }
}

impl MoveSource for OnyxBot {
    fn next_move(&mut self, ctx: &TurnContext, stop: &StopFlag) -> Move {
        let start = Instant::now();
        let me = code_of(ctx.side_to_move);
        let opp = code_of(ctx.side_to_move.opposite());
        let win = Win::from_rule(ctx.rule_set.win_rule);
        let mut grid = Grid::from_board(&ctx.board);

        // 空盘：天元。
        if grid.stone_count() == 0 {
            let center = Position::new(ctx.board.height() / 2, ctx.board.width() / 2);
            return Move::Place(center);
        }

        let budget = self.effective_budget(ctx);
        let deadline = start + budget;

        // 1) 立即取胜。
        let my_wins = grid.win_points(me, win);
        if let Some(&(r, c)) = best_by_order(&grid, &my_wins, me, opp) {
            return place(r, c);
        }

        // 2) 必堵对手成五点。
        let opp_wins = grid.win_points(opp, win);
        if let Some(&(r, c)) = best_by_order(&grid, &opp_wins, me, opp) {
            return place(r, c);
        }

        // 3) 自己的 VCF 强制胜。
        let vcf_deadline = (start + scale(budget, VCF_NUM)).min(deadline);
        if let Some((r, c)) = search::vcf_win_move(&mut grid, me, win, stop, vcf_deadline, u64::MAX)
        {
            return place(r, c);
        }

        // 3b) 自己的 VCT 强制胜（含活三连续威胁，VCF 找不到的双活三 / 四三链）。
        let threat_deadline = (start + scale(budget, VCT_NUM)).min(deadline);
        if let Some((r, c)) =
            vct::vct_win_move(&mut grid, me, win, stop, threat_deadline, VCT_NODE_CAP)
        {
            return place(r, c);
        }

        // 根候选：离任意子 ≤2 的空点，按启发排序后截断。
        let mut roots = grid.neighborhood_all(2);
        if roots.is_empty() {
            return ctx
                .legal_moves
                .first()
                .copied()
                .unwrap_or_else(|| Move::Place(Position::new(0, 0)));
        }
        roots.sort_by_key(|&(r, c)| std::cmp::Reverse(eval::order_key(&grid, r, c, me, opp)));
        roots.truncate(MAX_ROOTS);

        // 4) 防守过滤：剔除走完后让对手立即胜 / VCF 杀的着。
        let defense_deadline = (start + scale(budget, DEFENSE_NUM)).min(deadline);
        let mut safe = Vec::new();
        for &(r, c) in &roots {
            if Instant::now() >= defense_deadline {
                safe.push((r, c)); // 防守时间耗尽：剩余不再过滤，直接保留
                continue;
            }
            grid.place(r, c, me);
            let loses = grid.has_immediate_win(opp, win) || {
                let check = (Instant::now() + PER_DEFENSE_CHECK).min(defense_deadline);
                search::has_vcf(&mut grid, opp, win, stop, check, u64::MAX)
            };
            grid.unplace(r, c, me);
            if !loses {
                safe.push((r, c));
            }
        }
        let root_moves = if safe.is_empty() { roots } else { safe };

        // 5) 迭代加深 α-β 取最优。
        match search::search_best(&mut grid, me, win, &root_moves, stop, deadline, MAX_DEPTH) {
            Some((r, c)) => place(r, c),
            None => place(root_moves[0].0, root_moves[0].1),
        }
    }
}

/// 在若干点中按 `order_key` 取最高者（空集返回 `None`）。
fn best_by_order<'a>(
    grid: &Grid,
    points: &'a [(i32, i32)],
    me: u8,
    opp: u8,
) -> Option<&'a (i32, i32)> {
    points
        .iter()
        .max_by_key(|&&(r, c)| eval::order_key(grid, r, c, me, opp))
}

/// 按比例缩放时长。
fn scale(d: Duration, num: u32) -> Duration {
    let ms = u64::try_from(d.as_millis()).unwrap_or(u64::MAX);
    Duration::from_millis(ms.saturating_mul(u64::from(num)) / 100)
}

#[inline]
fn place(r: i32, c: i32) -> Move {
    Move::Place(Position::new(r as u8, c as u8))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use std::time::{Duration, Instant};

    use quintara_bot::{MoveSource, StopFlag};
    use quintara_model::{Board, Cell, Color, Move, Position, RuleSet, TurnContext};

    use crate::grid::{Grid, Win, BLACK, WHITE};
    use crate::{search, vct, OnyxBot};

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
            timeout_turn: Some(Duration::from_millis(500)),
            time_left: None,
        }
    }

    #[test]
    fn takes_the_immediate_win() {
        // 黑方四连，落两端任一成五。
        let board = board_with(&[(7, 7), (7, 8), (7, 9), (7, 10)], &[(0, 0), (0, 1)]);
        let mv = OnyxBot::new()
            .next_move(&ctx(board.clone(), Color::Black), &StopFlag::new())
            .position();
        let grid = Grid::from_board(&board);
        assert!(
            grid.would_win(i32::from(mv.row), i32::from(mv.col), BLACK, Win::Overline),
            "should win immediately, played {mv:?}"
        );
    }

    #[test]
    fn blocks_opponent_immediate_win() {
        // 白方四连 (7,7..10)，黑无威胁 → 必堵 (7,6) 或 (7,11)。
        let board = board_with(&[(0, 0), (0, 1)], &[(7, 7), (7, 8), (7, 9), (7, 10)]);
        let mv = OnyxBot::new()
            .next_move(&ctx(board, Color::Black), &StopFlag::new())
            .position();
        assert!(
            mv == Position::new(7, 6) || mv == Position::new(7, 11),
            "should block the four, played {mv:?}"
        );
    }

    #[test]
    fn finds_two_step_vcf() {
        // 黑被白 (7,4) 半堵：先 (7,8) 冲四逼白挡 (7,9)，再 (8,8) 成活四（双成五点）→ 强制胜。
        // 唯一的起手冲四是 (7,8)。
        let board = board_with(&[(7, 5), (7, 6), (7, 7), (5, 8), (6, 8)], &[(7, 4)]);
        let mut grid = Grid::from_board(&board);
        let deadline = Instant::now() + Duration::from_secs(5);
        let mv = search::vcf_win_move(
            &mut grid,
            BLACK,
            Win::Overline,
            &StopFlag::new(),
            deadline,
            u64::MAX,
        );
        assert_eq!(mv, Some((7, 8)), "expected VCF to start with (7,8)");
    }

    #[test]
    fn incremental_score_and_hash_match_full_recompute() {
        let blacks = [(7, 7), (7, 8), (8, 9), (5, 6), (9, 9), (6, 10)];
        let whites = [(7, 9), (8, 8), (6, 7), (9, 8), (5, 9)];

        // 基准：从 board 构建后全量重算。
        let mut base = Grid::from_board(&board_with(&blacks, &whites));
        base.prepare_search();
        let base_eval = base.eval_for(BLACK);
        let base_hash = base.hash();

        // 增量：空盘起，逐手 make 到相同局面。
        let mut inc = Grid::from_board(&Board::square(15));
        inc.prepare_search();
        for &(r, c) in &blacks {
            inc.make(i32::from(r), i32::from(c), BLACK);
        }
        for &(r, c) in &whites {
            inc.make(i32::from(r), i32::from(c), WHITE);
        }
        assert_eq!(inc.eval_for(BLACK), base_eval, "incremental score drifted");
        assert_eq!(inc.hash(), base_hash, "incremental hash drifted");

        // make/unmake 往返必须把 score 与 hash 精确还原。
        let (e0, h0) = (inc.eval_for(BLACK), inc.hash());
        inc.make(0, 0, BLACK);
        inc.unmake(0, 0, BLACK);
        assert_eq!(
            (inc.eval_for(BLACK), inc.hash()),
            (e0, h0),
            "round-trip drift"
        );
    }

    #[test]
    fn vct_finds_double_three() {
        // 黑两条「活二」交于 (7,8)：落 (7,8) 即成横、竖两条活三 → 双活三强制胜。
        let board = board_with(&[(7, 6), (7, 7), (5, 8), (6, 8)], &[]);
        let mut grid = Grid::from_board(&board);
        let deadline = Instant::now() + Duration::from_secs(5);
        let mv = vct::vct_win_move(
            &mut grid,
            BLACK,
            Win::Overline,
            &StopFlag::new(),
            deadline,
            1_000_000,
        );
        assert_eq!(
            mv,
            Some((7, 8)),
            "expected VCT to find the double-three at (7,8)"
        );
    }

    #[test]
    fn vct_rejects_non_forcing_open_two() {
        // 仅一条活二（黑先）：能做单活三，但单活三可被招架 → 非强制胜，不得误报。
        let board = board_with(&[(7, 7), (7, 8)], &[]);
        let mut grid = Grid::from_board(&board);
        let deadline = Instant::now() + Duration::from_secs(5);
        let mv = vct::vct_win_move(
            &mut grid,
            BLACK,
            Win::Overline,
            &StopFlag::new(),
            deadline,
            1_000_000,
        );
        assert_eq!(mv, None, "VCT must not claim a win from a single open two");
    }

    #[test]
    fn quiet_position_returns_legal_move() {
        // 单子局面不应崩溃，返回界内空点。
        let board = board_with(&[(7, 7)], &[]);
        let context = ctx(board.clone(), Color::White);
        let mv = OnyxBot::new()
            .next_move(&context, &StopFlag::new())
            .position();
        assert!(
            board.is_empty_at(mv),
            "played onto an occupied/oob cell {mv:?}"
        );
    }
}
