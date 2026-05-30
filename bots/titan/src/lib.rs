//! `quintara-bot-titan`：bitboard + α-β 搜索的五子棋 bot。
//!
//! 每手:候选 = 离子 ≤2 的合法点（空盘 → 天元）；能赢就赢、必堵对手成五（`is_win` 按规则）；
//! 试 VCF 算杀([`vcf`])命中即走；剔除会被对手 VCF 反杀的候选；其余用**迭代加深 α-β**
//! （[`search`]）在时间预算内前瞻,取最优着法,同分随机。
//!
//! 位棋盘地基见 [`bitboard`]（位线表示 + 增量哈希 / 评估 / 近邻计数）;搜索见 [`search`]
//! （α-β + 迭代加深 + Zobrist 置换表）;算杀见 [`vcf`]（连续冲四,攻防）。

mod bitboard;
mod search;
mod vcf;

use std::time::{Duration, Instant};

use bitboard::Bits;
use quintara_bot::{MoveSource, StopFlag};
use quintara_model::{Move, Position, TurnContext};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// 「什么限制都没设」时的兜底时间预算(纯限时 1s）。
const DEFAULT_BUDGET: Duration = Duration::from_secs(1);

/// 时间「实际不限」的兜底（只设了 `--depth` 且协议也没给时限时;搜索由深度收口）。
const UNBOUNDED_BUDGET: Duration = Duration::from_hours(24);

/// VCF 算杀占用的预算上限(很快返回;其余留给 α-β）。
const VCF_BUDGET: Duration = Duration::from_millis(100);

/// 防守 VCF 过滤的预算上限(整批候选合计;只是上限,通常远用不到——但太小会让后面候选的
/// `has_vcf` 中止、误判为「安全」而漏防,故给足）。
const DEFENSE_BUDGET: Duration = Duration::from_millis(200);

/// bitboard 搜索 bot。
///
/// `budget` / `max_depth` 均为 `None` = 未显式设置,据此决定时间策略(见 [`TitanBot::budget`]）:
/// 设了 time 用 time;只设了 depth 则时间只听协议;都没设兜底 1s。
pub struct TitanBot {
    rng: StdRng,
    /// 每手思考预算上限;`None` = 未设。仍受 `ctx.timeout_turn` / 本局剩余时间约束。
    budget: Option<Duration>,
    /// 迭代加深的最大深度;`None` = 未设(深到时间用完）。
    max_depth: Option<i32>,
}

impl TitanBot {
    #[must_use]
    pub fn new() -> Self {
        Self {
            rng: StdRng::from_entropy(),
            budget: None,
            max_depth: None,
        }
    }

    /// 设每手思考预算(上限)。
    #[must_use]
    pub fn with_budget(mut self, budget: Duration) -> Self {
        self.budget = Some(budget);
        self
    }

    /// 设最大搜索深度(≥1）。
    #[must_use]
    pub fn with_max_depth(mut self, depth: i32) -> Self {
        self.max_depth = Some(depth.max(1));
        self
    }

    /// 本手思考预算。策略(按优先级):
    /// - 设了 `budget`(--time）→ 用它,再被协议 `timeout_turn` 收窄;
    /// - 没设 --time 但协议给了 `timeout_turn` → **以 `timeout_turn` 为准**(协议每手限时即预算);
    /// - 只设了 `max_depth`(--depth,且协议也没限时)→ 时间不限(深度负责收口);
    /// - 什么都没设 → 兜底 [`DEFAULT_BUDGET`]。
    ///
    /// 末了再不超过本局剩余的 1/4。
    fn budget(&self, ctx: &TurnContext) -> Duration {
        let mut budget = match (self.budget, ctx.timeout_turn) {
            // 显式思考预算:用它,但不得越过协议每手限时。
            (Some(cap), Some(turn)) => cap.min(turn),
            (Some(cap), None) => cap,
            // 无显式预算但协议给了每手限时:以协议为准(别再被默认预算截断)。
            (None, Some(turn)) => turn,
            // 协议也没限时:只设了 --depth 则时间不限(深度收口),否则兜底默认预算。
            (None, None) => {
                if self.max_depth.is_some() {
                    UNBOUNDED_BUDGET
                } else {
                    DEFAULT_BUDGET
                }
            }
        };
        if let Some(left) = ctx.time_left {
            budget = budget.min(left / 4);
        }
        budget
    }

    fn pick(&mut self, points: &[Position]) -> Position {
        points[self.rng.gen_range(0..points.len())]
    }
}

impl Default for TitanBot {
    fn default() -> Self {
        Self::new()
    }
}

impl MoveSource for TitanBot {
    fn next_move(&mut self, ctx: &TurnContext, stop: &StopFlag) -> Move {
        // 预算时钟从进入本函数即起,涵盖 Bits 构造 / 候选生成 / win-block 扫描等前置开销,
        // 否则这部分时间不计入预算,墙钟会超出协议时限(紧时控下偶发自我判负)。
        let start = Instant::now();
        let me = ctx.side_to_move;
        let opponent = me.opposite();
        let rule_set = ctx.rule_set;
        let mut bits = Bits::from_board(&ctx.board);

        // 候选 = 离子 ≤2、按棋型排序的前 K 个合法点。
        let candidates = search::gen_candidates(&bits, me);

        // 空盘:天元,否则任意合法点。
        if candidates.is_empty() {
            let center = Position::new(ctx.board.height() / 2, ctx.board.width() / 2);
            if ctx.legal_moves.iter().any(|m| m.position() == center) {
                return Move::Place(center);
            }
            return ctx.legal_moves[0];
        }

        // 能赢就赢 / 必堵对手成五（成五点棋型分极高,必在候选内）。
        let wins: Vec<Position> = candidates
            .iter()
            .copied()
            .filter(|&pos| bits.would_win(me, pos, rule_set))
            .collect();
        if !wins.is_empty() {
            return Move::Place(self.pick(&wins));
        }
        let blocks: Vec<Position> = candidates
            .iter()
            .copied()
            .filter(|&pos| bits.would_win(opponent, pos, rule_set))
            .collect();
        if !blocks.is_empty() {
            return Move::Place(self.pick(&blocks));
        }

        // 整手硬上限。各阶段子预算(VCF/DEFENSE)只限制单阶段时长,但都不得越过它——否则
        // VCF+防守+反威胁会按 now()+子预算 逐段叠加到 ~VCF+DEFENSE+VCF=400ms,与 total 无关地
        // 冲过时限,造成自我判负。
        let total = self.budget(ctx);
        let deadline = start + total;

        // VCF 算杀:有连续冲四必胜则直接走（占用预算的一小部分,通常很快返回）。
        let vcf_deadline = (start + VCF_BUDGET).min(deadline);
        if let Some(m) = vcf::vcf_win_move(&mut bits, me, rule_set, stop, vcf_deadline) {
            return Move::Place(m);
        }

        // 防守:剔除「我走完后对手有连续冲四必杀」的候选。每个候选要新建一次 VCF 搜索（节点计数
        // 各自归零），故 `has_vcf` 内部的「按节点查 deadline」管不住跨候选的累加——必须在循环里
        // 逐候选查一次 wall clock,到点即停,否则一批候选能合计冲到几百毫秒。
        let def_deadline = (Instant::now() + DEFENSE_BUDGET).min(deadline);
        let mut safe: Vec<Position> = Vec::new();
        for &m in &candidates {
            if Instant::now() >= def_deadline {
                break; // 时间到:停止过滤;已确认的安全手照用,主搜索仍会评估其余。
            }
            let (r, c) = (i32::from(m.row), i32::from(m.col));
            bits.toggle(me, r, c);
            let opponent_mates = vcf::has_vcf(&mut bits, opponent, rule_set, stop, def_deadline);
            bits.toggle(me, r, c);
            if !opponent_mates {
                safe.push(m);
            }
        }

        // 着法集:有安全手 → 只在安全手里搜。无安全手(对手已有 VCF 必杀)→ **别去进攻**,
        // 改为占住对手发起 VCF 的那一手尽力打断——对 1-ply 对手常能破解,至少逼它另寻续杀
        // (旧实现此时 `search_all` 会扩张自己、放任对手成杀,正是对 sage 的败因)。
        let search_candidates: Vec<Position> = if safe.is_empty() {
            let threat_deadline = (Instant::now() + VCF_BUDGET).min(deadline);
            match vcf::vcf_win_move(&mut bits, opponent, rule_set, stop, threat_deadline) {
                Some(threat) => vec![threat],
                None => candidates,
            }
        } else {
            safe
        };

        // 迭代加深 α-β:在剩余预算内前瞻,取最优着法(同分随机)。
        let best = search::search_best(
            &mut bits,
            me,
            rule_set,
            &search_candidates,
            self.max_depth.unwrap_or(i32::MAX),
            stop,
            deadline,
        );
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
            timeout_turn: Some(Duration::from_millis(50)),
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
        let board = board_with(&[(7, 7), (7, 8), (7, 9), (7, 10)], &[(0, 0), (0, 1)]);
        let mv = TitanBot::new()
            .next_move(&ctx(board, Color::Black), &StopFlag::new())
            .position();
        assert!(
            mv == Position::new(7, 6) || mv == Position::new(7, 11),
            "should complete five, played {mv:?}"
        );
    }

    #[test]
    fn blocks_opponent_immediate_win() {
        let board = board_with(&[(0, 0), (0, 1)], &[(7, 7), (7, 8), (7, 9), (7, 10)]);
        let mv = TitanBot::new()
            .next_move(&ctx(board, Color::Black), &StopFlag::new())
            .position();
        assert!(
            mv == Position::new(7, 6) || mv == Position::new(7, 11),
            "should block the four, played {mv:?}"
        );
    }

    #[test]
    fn budget_respects_protocol_timeout_turn() {
        // 没设 :time:协议 timeout_turn 即预算(不再被默认 1s 截断)——回归 #budget bug。
        let mut c = ctx(Board::square(15), Color::Black);
        c.timeout_turn = Some(Duration::from_secs(2));
        assert_eq!(TitanBot::new().budget(&c), Duration::from_secs(2));
        // 设了 :time 且小于协议限时:用 :time。
        assert_eq!(
            TitanBot::new()
                .with_budget(Duration::from_millis(300))
                .budget(&c),
            Duration::from_millis(300)
        );
        // :time 大于协议限时:被协议收窄。
        assert_eq!(
            TitanBot::new()
                .with_budget(Duration::from_secs(5))
                .budget(&c),
            Duration::from_secs(2)
        );
        // 协议无限时(serve 收到 INFO 前):只设 --depth 则时间不限。
        c.timeout_turn = None;
        assert_eq!(
            TitanBot::new().with_max_depth(8).budget(&c),
            UNBOUNDED_BUDGET
        );
        // 什么都没设、协议也无限时:兜底默认预算。
        assert_eq!(TitanBot::new().budget(&c), DEFAULT_BUDGET);
    }

    /// 性能基准（手动跑，**务必 release**）：
    /// `cargo test --release -p quintara-bot-titan --lib search_node_rate -- --ignored --nocapture`
    /// 安静局面（无活三/冲四，绕开 VCF）定深 8 全搜的墙钟时间，越低越快。改提速时做前后对照。
    #[test]
    #[ignore = "perf benchmark; run with --release --ignored --nocapture"]
    fn search_node_rate() {
        let board = board_with(&[(7, 7), (8, 8)], &[(7, 8), (8, 7)]);
        let mut c = ctx(board, Color::Black);
        c.timeout_turn = Some(Duration::from_secs(30));
        let bot = || TitanBot::new().with_max_depth(8);
        // 预热一次（缓存/分支预测），再计时三次取最小。
        let _ = bot().next_move(&c, &StopFlag::new());
        let mut best = u128::MAX;
        for _ in 0..3 {
            let t = std::time::Instant::now();
            let _ = bot().next_move(&c, &StopFlag::new());
            best = best.min(t.elapsed().as_millis());
        }
        eprintln!("search_node_rate: depth-8 quiet search = {best} ms (min of 3)");
    }
}
