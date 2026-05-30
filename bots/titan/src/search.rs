//! Stage B 搜索:负极大 + α-β + 迭代加深 + 时间预算,跑在位棋盘上。
//!
//! 候选限于「离子 ≤2」的空点,按棋型分排序取前 `TOP_K`(剪枝高效)。叶子用全盘静态评估;
//! 落子成五 = 终局(浅胜优先)。`make`/`unmake` 即位棋盘的 set/clear。时间到 / `StopFlag`
//! 翻起即中止,根部用上一层「完整跑完」的结果。

use std::collections::HashMap;
use std::time::Instant;

use quintara_bot::StopFlag;
use quintara_model::{Color, Position, RuleSet};

use crate::bitboard::Bits;

const WIN: i64 = 1_000_000;
const TOP_K: usize = 12;
/// killer 表的最大层数（搜索深度极少超过此值;越界层不记 killer）。
const MAX_PLY: usize = 64;

/// 置换表条目的分值界限。
#[derive(Clone, Copy)]
enum Bound {
    Exact,
    Lower, // 真值 ≥ value（发生过 β 剪枝）
    Upper, // 真值 ≤ value（未抬过 α）
}

#[derive(Clone, Copy)]
struct TtEntry {
    depth: i32,
    value: i64,
    bound: Bound,
    /// 该结点搜出的最佳着法,供后续（即便深度不足以直接采用其分值）做着法排序。
    best: Option<Position>,
}

/// 把 `m` 换到 `candidates[front..]` 的最前并返回新的 front;`m` 不在其中则原样返回。
/// 用于把 TT 最佳着 / killer 提到候选表前列(命中后更早 β 剪枝)。
fn bring_front(candidates: &mut [Position], front: usize, m: Option<Position>) -> usize {
    if let Some(m) = m {
        if let Some(i) = candidates[front..].iter().position(|&x| x == m) {
            candidates.swap(front, front + i);
            return front + 1;
        }
    }
    front
}

/// 区分行动方的 TT key（同盘面不同走子方是不同结点）。
fn tt_key(bits: &Bits, side: Color) -> u64 {
    match side {
        Color::Black => bits.hash(),
        Color::White => bits.hash() ^ 0xD1B5_4A32_D192_ED03,
    }
}

fn pos_to_rc(pos: Position) -> (i32, i32) {
    (i32::from(pos.row), i32::from(pos.col))
}

/// 生成候选:离子 ≤2 的空点,按 [`Bits::candidate_score`]（本色棋型 + 对手棋型）降序取前 `TOP_K`。
/// 只读(不落子)。
pub(crate) fn gen_candidates(bits: &Bits, side: Color) -> Vec<Position> {
    let mut scored: Vec<(i64, Position)> = bits
        .relevant_empties()
        .into_iter()
        .map(|pos| (bits.candidate_score(side, pos), pos))
        .collect();
    scored.sort_by_key(|&(key, _)| std::cmp::Reverse(key));
    scored.truncate(TOP_K);
    scored.into_iter().map(|(_, pos)| pos).collect()
}

struct Searcher<'a> {
    rule_set: RuleSet,
    stop: &'a StopFlag,
    deadline: Instant,
    aborted: bool,
    nodes: u64,
    tt: HashMap<u64, TtEntry>,
    /// 每层两个 killer 着法（造成 β 剪枝的着法,下次该层优先试）。
    killers: Vec<[Option<Position>; 2]>,
}

impl Searcher<'_> {
    /// 是否该停（周期性查时间 / `StopFlag`）。
    fn out_of_time(&mut self) -> bool {
        self.nodes += 1;
        if self.nodes.is_multiple_of(1024)
            && (self.stop.should_stop() || Instant::now() >= self.deadline)
        {
            self.aborted = true;
        }
        self.aborted
    }

    /// 取某层的两个 killer 着法（越界层返回空）。
    fn killers_at(&self, ply: i64) -> [Option<Position>; 2] {
        usize::try_from(ply)
            .ok()
            .and_then(|p| self.killers.get(p).copied())
            .unwrap_or([None, None])
    }

    /// 记一个造成 β 剪枝的 killer 着法（新着挤到 slot[0],旧的退到 slot[1]）。
    fn record_killer(&mut self, ply: i64, m: Position) {
        if let Ok(p) = usize::try_from(ply) {
            if let Some(slot) = self.killers.get_mut(p) {
                if slot[0] != Some(m) {
                    slot[1] = slot[0];
                    slot[0] = Some(m);
                }
            }
        }
    }

    fn eval(bits: &Bits, side: Color) -> i64 {
        bits.position_score(side) - bits.position_score(side.opposite())
    }

    /// 负极大 + α-β。返回从 `side` 视角的分；`ply` = 距根步数(浅胜优先)。
    fn negamax(
        &mut self,
        bits: &mut Bits,
        side: Color,
        depth: i32,
        mut alpha: i64,
        mut beta: i64,
        ply: i64,
    ) -> i64 {
        if self.out_of_time() {
            return 0; // 中止:返回值会被根部丢弃
        }
        if depth == 0 {
            return Self::eval(bits, side);
        }
        // 置换表探测:深度足够时直接用缓存；不论深度,取其最佳着用于排序。
        let key = tt_key(bits, side);
        let alpha_orig = alpha;
        let mut tt_move = None;
        if let Some(entry) = self.tt.get(&key).copied() {
            tt_move = entry.best;
            if entry.depth >= depth {
                match entry.bound {
                    Bound::Exact => return entry.value,
                    Bound::Lower => alpha = alpha.max(entry.value),
                    Bound::Upper => beta = beta.min(entry.value),
                }
                if alpha >= beta {
                    return entry.value;
                }
            }
        }
        let mut candidates = gen_candidates(bits, side);
        if candidates.is_empty() {
            return Self::eval(bits, side);
        }
        // 着法排序:TT 最佳着 → 两个 killer → 其余按棋型分(更早 β 剪枝)。
        let killers = self.killers_at(ply);
        let mut front = bring_front(&mut candidates, 0, tt_move);
        front = bring_front(&mut candidates, front, killers[0]);
        let _ = bring_front(&mut candidates, front, killers[1]);

        let opponent = side.opposite();
        let mut best = i64::MIN / 2;
        let mut best_move = None;
        for pos in candidates {
            let (r, c) = pos_to_rc(pos);
            bits.toggle(side, r, c);
            let val = if bits.is_win(side, r, c, self.rule_set) {
                WIN - ply
            } else {
                -self.negamax(bits, opponent, depth - 1, -beta, -alpha, ply + 1)
            };
            bits.toggle(side, r, c);
            if self.aborted {
                return best;
            }
            if val > best {
                best = val;
                best_move = Some(pos);
            }
            alpha = alpha.max(best);
            if alpha >= beta {
                self.record_killer(ply, pos); // 造成剪枝的着法记为 killer
                break;
            }
        }
        // 存表:据 best 相对原窗口判界限,并记最佳着供后续排序。
        let bound = if best <= alpha_orig {
            Bound::Upper
        } else if best >= beta {
            Bound::Lower
        } else {
            Bound::Exact
        };
        self.tt.insert(
            key,
            TtEntry {
                depth,
                value: best,
                bound,
                best: best_move,
            },
        );
        best
    }
}

/// 迭代加深搜索,返回**最深完整层**里并列最优的候选集（同分由调用方随机取一）。
pub(crate) fn search_best(
    bits: &mut Bits,
    side: Color,
    rule_set: RuleSet,
    root_candidates: &[Position],
    max_depth: i32,
    stop: &StopFlag,
    deadline: Instant,
) -> Vec<Position> {
    if root_candidates.len() <= 1 {
        return root_candidates.to_vec();
    }
    let mut searcher = Searcher {
        rule_set,
        stop,
        deadline,
        aborted: false,
        nodes: 0,
        tt: HashMap::new(),
        killers: vec![[None, None]; MAX_PLY],
    };
    let opponent = side.opposite();
    let mut result = root_candidates.to_vec();
    // 根着法顺序:每层把上一层的最佳着提到最前（PV-first → 更早抬高 alpha、子结点剪枝更紧）。
    let mut order = root_candidates.to_vec();

    for depth in 1..=max_depth {
        let mut alpha = i64::MIN / 2;
        let beta = i64::MAX / 2;
        let mut best_val = i64::MIN / 2;
        let mut best: Vec<Position> = Vec::new();
        for &pos in &order {
            let (r, c) = pos_to_rc(pos);
            bits.toggle(side, r, c);
            let val = if bits.is_win(side, r, c, rule_set) {
                WIN
            } else {
                -searcher.negamax(bits, opponent, depth - 1, -beta, -alpha, 1)
            };
            bits.toggle(side, r, c);
            if searcher.aborted {
                break;
            }
            if val > best_val {
                best_val = val;
                best.clear();
                best.push(pos);
            } else if val == best_val {
                best.push(pos);
            }
            alpha = alpha.max(val);
        }
        if searcher.aborted {
            break; // 本层未跑完:丢弃,用上一层结果
        }
        let winning = best_val >= WIN / 2;
        // 只在「偶数层」(对手已应手、评估真实)采纳结果;奇数层评估带 tempo 虚高(我方刚出招、
        // 对手未应),只用来热身 TT / 排序,不作数。找到强制胜则无论奇偶都采纳(确凿杀棋,非虚高)。
        if depth % 2 == 0 || winning {
            result.clone_from(&best);
        }
        if winning {
            break;
        }
        if stop.should_stop() || Instant::now() >= deadline {
            break;
        }
        // 下一层先试本层算出的最佳着(即便该层结果未采纳,顺序信息仍有用)。
        bring_front(&mut order, 0, best.first().copied());
    }
    result
}
