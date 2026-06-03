//! 搜索：VCF 连续四杀 + 时间预算内的迭代加深 α-β。
//!
//! - [`vcf_win_move`] / [`has_vcf`]：进攻方只走「冲四」（每步逼对手唯一挡点），证明 / 否证强制
//!   胜。声明胜时不假设对手配合——对手要么挡唯一成五点、要么自己已能成五（此时该线失败）。
//! - [`search_best`]：安静局面的兜底，negamax + α-β + 迭代加深，受 `deadline` / `stop` 约束。

use std::collections::HashMap;
use std::time::Instant;

use quintara_bot::StopFlag;

use crate::eval::{evaluate, order_key};
use crate::grid::{Grid, Win, BLACK};

/// 杀棋分值（远大于任何静态评估）。
pub const WIN: i32 = 10_000_000;
/// 判定为杀的阈值。
pub const MATE: i32 = WIN - 10_000;
/// 每个内部节点展开的候选上限。
const TOP_K: usize = 16;
/// VCF 最大进攻层数（仍以时间为先约束）。
const VCF_MAX_DEPTH: i32 = 40;
/// negamax 多久检查一次时钟（节点数）。
const TIME_CHECK_MASK: u64 = 255;
/// 区分行棋方的 Zobrist 侧键（白方时 xor 进探查键）。
const SIDE_KEY: u64 = 0x9E37_79B9_7F4A_7C15;

/// 置换表的界限类型。
#[derive(Clone, Copy)]
enum Bound {
    Exact,
    Lower,
    Upper,
}

/// 置换表条目。
#[derive(Clone, Copy)]
struct TtEntry {
    depth: i32,
    value: i32,
    bound: Bound,
    best: Option<(i32, i32)>,
}

/// 行棋方对应的探查键偏移。
#[inline]
fn side_key(side: u8) -> u64 {
    if side == BLACK {
        0
    } else {
        SIDE_KEY
    }
}
/// VCF 多久检查一次时钟。VCF 单节点很「重」（`four_moves` → 嵌套 `count_win_points` 邻域扫描），
/// 故取远更密的间隔，把超时溢出从约 100ms 压到约 10ms。
const VCF_TIME_MASK: u64 = 31;

/// VCF 搜索器。
struct Vcf<'a> {
    win: Win,
    stop: &'a StopFlag,
    deadline: Instant,
    nodes: u64,
    aborted: bool,
}

impl Vcf<'_> {
    fn out_of_time(&mut self) -> bool {
        self.nodes += 1;
        if self.nodes & VCF_TIME_MASK == 0
            && (Instant::now() >= self.deadline || self.stop.should_stop())
        {
            self.aborted = true;
        }
        self.aborted
    }

    /// 进攻方走棋：存在强制胜返回 true。
    fn attack(&mut self, grid: &mut Grid, atk: u8, def: u8, depth: i32) -> bool {
        if self.out_of_time() {
            return false;
        }
        let mut moves = grid.four_moves(atk, self.win);
        // 成五点多的（双四 / 活四）优先。
        moves.sort_by_key(|&(_, wins)| std::cmp::Reverse(wins));
        for ((r, c), wins) in moves {
            grid.place(r, c, atk);
            if wins >= 2 {
                // 不可挡的双威胁——但须确认对手不能先成五。
                let opp_wins = grid.has_immediate_win(def, self.win);
                grid.unplace(r, c, atk);
                if !opp_wins {
                    return true;
                }
                continue;
            }
            // 单冲四：对手若能立即成五则反杀，本线作废。
            if grid.has_immediate_win(def, self.win) {
                grid.unplace(r, c, atk);
                continue;
            }
            // 否则对手被迫挡唯一成五点。
            let wp = grid.win_points(atk, self.win);
            if wp.len() != 1 {
                grid.unplace(r, c, atk);
                continue;
            }
            let (br, bc) = wp[0];
            grid.place(br, bc, def);
            let res = depth > 0 && self.attack(grid, atk, def, depth - 1);
            grid.unplace(br, bc, def);
            grid.unplace(r, c, atk);
            if res {
                return true;
            }
            if self.aborted {
                return false;
            }
        }
        false
    }
}

/// 找到 `atk` 的一步 VCF 制胜着；无则 `None`。
#[must_use]
pub fn vcf_win_move(
    grid: &mut Grid,
    atk: u8,
    win: Win,
    stop: &StopFlag,
    deadline: Instant,
) -> Option<(i32, i32)> {
    let def = other(atk);
    let mut vcf = Vcf {
        win,
        stop,
        deadline,
        nodes: 0,
        aborted: false,
    };
    let mut moves = grid.four_moves(atk, win);
    moves.sort_by_key(|&(_, wins)| std::cmp::Reverse(wins));
    for ((r, c), wins) in moves {
        grid.place(r, c, atk);
        let res = if wins >= 2 {
            !grid.has_immediate_win(def, win)
        } else if grid.has_immediate_win(def, win) {
            false
        } else {
            match grid.win_points(atk, win).first().copied() {
                Some((br, bc)) => {
                    grid.place(br, bc, def);
                    let r = vcf.attack(grid, atk, def, VCF_MAX_DEPTH);
                    grid.unplace(br, bc, def);
                    r
                }
                None => false,
            }
        };
        grid.unplace(r, c, atk);
        if res {
            return Some((r, c));
        }
        if vcf.aborted {
            return None;
        }
    }
    None
}

/// `atk` 是否存在 VCF 强制胜（用于防守过滤）。
#[must_use]
pub fn has_vcf(grid: &mut Grid, atk: u8, win: Win, stop: &StopFlag, deadline: Instant) -> bool {
    vcf_win_move(grid, atk, win, stop, deadline).is_some()
}

/// negamax + α-β 搜索器。
struct Searcher<'a> {
    win: Win,
    stop: &'a StopFlag,
    deadline: Instant,
    nodes: u64,
    aborted: bool,
    /// 置换表：跨迭代加深各层共享，命中即剪枝 / 提着。
    tt: HashMap<u64, TtEntry>,
}

impl Searcher<'_> {
    fn out_of_time(&mut self) -> bool {
        self.nodes += 1;
        if self.nodes & TIME_CHECK_MASK == 0
            && (Instant::now() >= self.deadline || self.stop.should_stop())
        {
            self.aborted = true;
        }
        self.aborted
    }

    fn negamax(
        &mut self,
        grid: &mut Grid,
        side: u8,
        depth: i32,
        mut alpha: i32,
        mut beta: i32,
        ply: i32,
    ) -> i32 {
        if self.out_of_time() {
            return 0;
        }
        if depth == 0 {
            return evaluate(grid, side, self.win);
        }

        // 置换表探查：足够深的条目可直接给界 / 剪枝，并提供首选着。
        let key = grid.hash() ^ side_key(side);
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

        let opp = other(side);
        let mut cands = grid.neighborhood_all(2);
        if cands.is_empty() {
            return evaluate(grid, side, self.win);
        }
        cands.sort_by_key(|&(r, c)| std::cmp::Reverse(order_key(grid, r, c, side, opp)));
        // 置换表首选着提到最前（在截断前，确保不被裁掉）。
        if let Some(m) = tt_move {
            if let Some(pos) = cands.iter().position(|&x| x == m) {
                cands.swap(0, pos);
            }
        }
        cands.truncate(TOP_K);

        let mut best = -WIN;
        let mut best_move = None;
        for (r, c) in cands {
            let val = if grid.would_win(r, c, side, self.win) {
                WIN - ply
            } else {
                grid.make(r, c, side);
                let v = -self.negamax(grid, opp, depth - 1, -beta, -alpha, ply + 1);
                grid.unmake(r, c, side);
                v
            };
            if self.aborted {
                return best.max(val);
            }
            if val > best {
                best = val;
                best_move = Some((r, c));
            }
            if best > alpha {
                alpha = best;
            }
            if alpha >= beta {
                break;
            }
        }

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

/// 在 `root_moves` 中迭代加深选最佳着；受 `deadline` / `stop` 约束。空集返回 `None`。
#[must_use]
pub fn search_best(
    grid: &mut Grid,
    me: u8,
    win: Win,
    root_moves: &[(i32, i32)],
    stop: &StopFlag,
    deadline: Instant,
    max_depth: i32,
) -> Option<(i32, i32)> {
    if root_moves.is_empty() {
        return None;
    }
    // 全量算好基准局面的增量 score / hash，之后 make/unmake 增量维护。
    grid.prepare_search();
    let opp = other(me);
    let mut best = root_moves[0];
    let mut order = root_moves.to_vec();
    let mut searcher = Searcher {
        win,
        stop,
        deadline,
        nodes: 0,
        aborted: false,
        tt: HashMap::new(),
    };

    for depth in 1..=max_depth {
        searcher.nodes = 0;
        searcher.aborted = false;
        let mut alpha = -WIN;
        let beta = WIN;
        let mut local_best = order[0];
        let mut local_val = -WIN;
        let mut completed = true;

        for &(r, c) in &order {
            let val = if grid.would_win(r, c, me, win) {
                WIN
            } else {
                grid.make(r, c, me);
                let v = -searcher.negamax(grid, opp, depth - 1, -beta, -alpha, 1);
                grid.unmake(r, c, me);
                v
            };
            if searcher.aborted {
                completed = false;
                break;
            }
            if val > local_val {
                local_val = val;
                local_best = (r, c);
            }
            if local_val > alpha {
                alpha = local_val;
            }
        }

        if completed {
            best = local_best;
            // 把本层最优着提前，给下一层更紧的窗口。
            if let Some(pos) = order.iter().position(|&p| p == local_best) {
                order.swap(0, pos);
            }
            if local_val >= MATE {
                break; // 已找到杀，无需更深
            }
        } else {
            break; // 本层不完整，沿用上一完整层结果
        }
    }
    Some(best)
}

#[inline]
fn other(color: u8) -> u8 {
    use crate::grid::{BLACK, WHITE};
    if color == BLACK {
        WHITE
    } else {
        BLACK
    }
}
