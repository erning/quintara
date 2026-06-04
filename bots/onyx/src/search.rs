//! 搜索：VCF 连续四杀 + 时间预算内的迭代加深 α-β。
//!
//! - [`vcf_win_move`] / [`has_vcf`]：进攻方只走「冲四」（每步逼对手唯一挡点），证明 / 否证强制
//!   胜。声明胜时不假设对手配合——对手要么挡唯一成五点、要么自己已能成五（此时该线失败）。
//! - [`search_best`]：安静局面的兜底，negamax + α-β + 迭代加深，受 `deadline` / `stop` 约束。

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
/// 叶子 VCF 静态延伸的单次节点上限（仅在「强制手」之后的叶子触发，控开销）。
const QUIESCENCE_VCF_NODES: u64 = 800;
/// negamax 多久检查一次时钟（节点数）。迭代加深每层都把 `nodes` 清零，故除周期检查外，
/// 每层第 1 个节点也强制读钟（见 [`Searcher::out_of_time`]）：否则新层要先白跑这么多节点才发现超时。
const TIME_CHECK_MASK: u64 = 63;
/// 区分行棋方的 Zobrist 侧键（白方时 xor 进探查键）。
const SIDE_KEY: u64 = 0x9E37_79B9_7F4A_7C15;

/// 置换表的界限类型。
#[derive(Clone, Copy)]
enum Bound {
    Exact,
    Lower,
    Upper,
}

/// 置换表条目。`key` 是完整 Zobrist 键，用于在定长表里校验槽位归属（避免索引截断带来的伪命中）。
#[derive(Clone, Copy)]
struct TtEntry {
    key: u64,
    depth: i32,
    value: i32,
    bound: Bound,
    best: Option<(i32, i32)>,
}

/// 置换表索引位数：定长直接映射数组，`2^TT_BITS` 个槽位（每手新建一次，约 20MB）。
const TT_BITS: u32 = 19;

/// 定长直接映射置换表。键已是高质量 Zobrist 哈希，故直接 `key & mask` 索引、整键校验、冲突即覆盖——
/// 省掉 `HashMap` 的 `SipHash` 重哈希与扩容 rehash（搜索热路径上每节点都 probe / store）。
struct Tt {
    slots: Vec<Option<TtEntry>>,
    mask: usize,
}

impl Tt {
    fn new() -> Self {
        let size = 1usize << TT_BITS;
        Self {
            slots: vec![None; size],
            mask: size - 1,
        }
    }

    /// 整键校验的探查：槽位为空或键不符都视为未命中。
    #[inline]
    fn probe(&self, key: u64) -> Option<&TtEntry> {
        match &self.slots[key as usize & self.mask] {
            Some(e) if e.key == key => Some(e),
            _ => None,
        }
    }

    /// 写入（always-replace：冲突直接覆盖旧槽）。
    #[inline]
    fn store(&mut self, entry: TtEntry) {
        self.slots[entry.key as usize & self.mask] = Some(entry);
    }
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
/// VCF 多久检查一次时钟。VCF 单节点很「重」（`four_moves` → 嵌套成五点扫描），
/// 故取远更密的间隔，把超时溢出从约 100ms 压到约 10ms。
const VCF_TIME_MASK: u64 = 31;

/// VCF 搜索器。
struct Vcf<'a> {
    win: Win,
    stop: &'a StopFlag,
    deadline: Instant,
    nodes: u64,
    /// 节点上限：根搜索传 `u64::MAX`，叶子静态延伸传小值以控开销。
    node_cap: u64,
    aborted: bool,
}

impl Vcf<'_> {
    fn out_of_time(&mut self) -> bool {
        self.nodes += 1;
        // node 1 也读钟：叶子静态延伸 / 防守否证为每个候选新建一个 Vcf（nodes 从 0 起）。若全局
        // deadline 已过，必须在第 1 个节点就中止，否则每次都先白跑 ~VCF_TIME_MASK 个「重」节点
        // （four_moves→嵌套邻域扫描），在 α-β 叶子处成百上千次累积，造成可观的超时溢出。
        let timed = (self.nodes == 1 || self.nodes & VCF_TIME_MASK == 0)
            && (Instant::now() >= self.deadline || self.stop.should_stop());
        if self.nodes > self.node_cap || timed {
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
    node_cap: u64,
) -> Option<(i32, i32)> {
    // 进攻方若已有立即成五点，本身即强制胜，直接返回。这也保证了下方 `four_moves` 的调用前提
    // （进攻方落子前无成五点），使其内部「只数穿过落点的新成五点」的局部统计严格等价于全盘统计。
    if let Some(&p) = grid.win_points(atk, win).first() {
        return Some(p);
    }
    let def = other(atk);
    let mut vcf = Vcf {
        win,
        stop,
        deadline,
        nodes: 0,
        node_cap,
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

/// `atk` 是否存在 VCF 强制胜（用于防守过滤 / 叶子静态延伸）。
#[must_use]
pub fn has_vcf(
    grid: &mut Grid,
    atk: u8,
    win: Win,
    stop: &StopFlag,
    deadline: Instant,
    node_cap: u64,
) -> bool {
    vcf_win_move(grid, atk, win, stop, deadline, node_cap).is_some()
}

/// negamax + α-β 搜索器。
struct Searcher<'a> {
    win: Win,
    stop: &'a StopFlag,
    deadline: Instant,
    nodes: u64,
    aborted: bool,
    /// 置换表：跨迭代加深各层共享，命中即剪枝 / 提着。
    tt: Tt,
    /// 杀手着：每层最近两个引发 β 截断的着，用于同层兄弟节点的着法排序。
    killers: Vec<[Option<(i32, i32)>; 2]>,
}

impl Searcher<'_> {
    fn out_of_time(&mut self) -> bool {
        self.nodes += 1;
        // node 1 也读钟：迭代加深每层把 nodes 清零，否则新层要先跑满一个掩码周期才发现 deadline
        // 已过——一个昂贵的深层会无谓地先跑数十个节点。第 1 个节点即检查可整体掐掉这种新层溢出。
        if (self.nodes == 1 || self.nodes & TIME_CHECK_MASK == 0)
            && (Instant::now() >= self.deadline || self.stop.should_stop())
        {
            self.aborted = true;
        }
        self.aborted
    }

    /// 记录引发 β 截断的杀手着（新着滑入槽 0，旧着退到槽 1；去重）。
    fn record_killer(&mut self, ply: i32, mv: (i32, i32)) {
        let p = ply as usize;
        if p < self.killers.len() && self.killers[p][0] != Some(mv) {
            self.killers[p][1] = self.killers[p][0];
            self.killers[p][0] = Some(mv);
        }
    }

    // NOTE: 标准 negamax 的参数列表（盘面 / 行棋方 / 深度 / α / β / ply / 强制手标记）天然较长，
    // 拆成 struct 反而更晦涩；此处局部豁免 too_many_arguments。
    #[allow(clippy::too_many_arguments)]
    fn negamax(
        &mut self,
        grid: &mut Grid,
        side: u8,
        depth: i32,
        mut alpha: i32,
        mut beta: i32,
        ply: i32,
        forcing: bool,
    ) -> i32 {
        if self.out_of_time() {
            return 0;
        }
        if depth == 0 {
            // 强制手后的叶子做 VCF 静态延伸：行棋方若有连续四杀（sound），视为必胜叶子，
            // 让 α-β 看穿静态地平线外的强制胜 / 负。只在「强制手」后触发以控开销。
            if forcing
                && has_vcf(
                    grid,
                    side,
                    self.win,
                    self.stop,
                    self.deadline,
                    QUIESCENCE_VCF_NODES,
                )
            {
                return WIN - ply;
            }
            return evaluate(grid, side, self.win);
        }

        // 置换表探查：足够深的条目可直接给界 / 剪枝，并提供首选着。
        let key = grid.hash() ^ side_key(side);
        let alpha_orig = alpha;
        let mut tt_move = None;
        if let Some(entry) = self.tt.probe(key).copied() {
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
        let raw = grid.neighborhood_all(2);
        if raw.is_empty() {
            return evaluate(grid, side, self.win);
        }
        let ordered = self.order_moves(grid, raw, side, opp, ply, tt_move);

        let mut best = -WIN;
        let mut best_move = None;
        for (r, c) in ordered {
            let val = if grid.would_win(r, c, side, self.win) {
                WIN - ply
            } else {
                grid.make(r, c, side);
                let child_forcing = grid.creates_threat(r, c, side);
                let v = -self.negamax(grid, opp, depth - 1, -beta, -alpha, ply + 1, child_forcing);
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
                self.record_killer(ply, (r, c));
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
        self.tt.store(TtEntry {
            key,
            depth,
            value: best,
            bound,
            best: best_move,
        });
        best
    }

    /// 着法排序：候选按启发分降序，再把首选着（置换表着 + 两个杀手着）提到最前——在截断到
    /// `TOP_K` 之前确保不被裁掉。返回排好序、已截断的着法表。
    fn order_moves(
        &self,
        grid: &Grid,
        mut cands: Vec<(i32, i32)>,
        side: u8,
        opp: u8,
        ply: i32,
        tt_move: Option<(i32, i32)>,
    ) -> Vec<(i32, i32)> {
        cands.sort_by_key(|&(r, c)| std::cmp::Reverse(order_key(grid, r, c, side, opp)));
        let killers = self
            .killers
            .get(ply as usize)
            .copied()
            .unwrap_or([None, None]);
        let priorities = [tt_move, killers[0], killers[1]];
        let mut ordered: Vec<(i32, i32)> = Vec::with_capacity(cands.len());
        for &p in priorities.iter().flatten() {
            if cands.contains(&p) && !ordered.contains(&p) {
                ordered.push(p);
            }
        }
        // 候选集本身已去重（邻域 stamp 去重），故第二轮只需排除已置顶的 ≤3 个优先着——
        // 对该前缀判断即可，避免在随 push 增长的 `ordered` 上做 O(n²) 线性查找。
        let n_pri = ordered.len();
        for &m in &cands {
            if !ordered[..n_pri].contains(&m) {
                ordered.push(m);
            }
        }
        ordered.truncate(TOP_K);
        ordered
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
        tt: Tt::new(),
        killers: vec![[None, None]; (max_depth as usize) + 2],
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
                let child_forcing = grid.creates_threat(r, c, me);
                let v = -searcher.negamax(grid, opp, depth - 1, -beta, -alpha, 1, child_forcing);
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
