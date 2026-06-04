//! VCT（连续威胁取胜）：根节点用的威胁空间搜索。进攻方只走「威胁手」（冲四 **或** 活三），
//! 把对手的应手压到必然回应，证明强制胜。VCF 只走四，找不到双活三 / 四三链这类胜——VCT 补上。
//!
//! **Soundness（声明胜必为真胜）**：
//! - 进攻方走出活四 / 双四（≥2 个成五点）：对手挡不全 → 胜（前提：对手当下不能先成五）。
//! - 走出（单）冲四（恰 1 个成五点）：对手唯一应手是挡该点（否则被成五），递归。
//! - 走出活三（0 个成五点、但存在「成活四的落点」）：对手必须招架。**关键**：能招架的着只可能是
//!   ①占据进攻方的某个「成活四落点」，或 ②自己冲四 / 成五反先。其余着都无法阻止进攻方做出活四。
//!   故防守应手的**完备超集** = `{进攻方成活四落点} ∪ {防守方冲四/成五点}`；若进攻方对该集合**所有**
//!   应手都仍能强制胜，则确为真胜。此完备性论证由单测兜底（已知双活三必胜、安静局面不误报）。
//!
//! 仅在根节点每手跑一次（不碰 α-β 叶子成本），并受 `node_cap` / `deadline` 双重约束。

use std::time::Instant;

use quintara_bot::StopFlag;

use crate::grid::{Grid, Win, BLACK, WHITE};

/// VCT 最大进攻层数（每层 = 进攻一手 + 防守回应）。
const VCT_MAX_DEPTH: i32 = 9;
/// 多久检查一次时钟（节点数）。VCT 的单节点极重（每个 `attack` 跑一遍 `threat_moves` 全邻域
/// 扫描 + 反复 `win_points` / `four_moves`），故取很密的间隔；读钟开销相对节点成本可忽略。
const TIME_MASK: u64 = 7;

struct Vct<'a> {
    win: Win,
    stop: &'a StopFlag,
    deadline: Instant,
    nodes: u64,
    node_cap: u64,
    aborted: bool,
}

impl Vct<'_> {
    fn out_of_budget(&mut self) -> bool {
        self.nodes += 1;
        // node 1 也读钟：VCT 在根每手新建一次，且常在前序阶段（VCF）已耗到其 deadline 之后才进入；
        // 不在第 1 个节点检查就会先白跑一个掩码周期的「重」节点，造成可观溢出。
        let timed = (self.nodes == 1 || self.nodes & TIME_MASK == 0)
            && (Instant::now() >= self.deadline || self.stop.should_stop());
        if self.nodes > self.node_cap || timed {
            self.aborted = true;
        }
        self.aborted
    }

    /// OR 节点：进攻方走棋，存在威胁手强制胜则 true。
    fn attack(&mut self, g: &mut Grid, atk: u8, def: u8, depth: i32) -> bool {
        if self.out_of_budget() || depth <= 0 {
            return false;
        }
        // 快速胜判（也是关键剪枝）：进攻方若现在就能成五、或能做出活四（对手当下无法先成五），即胜。
        if !g.win_points(atk, self.win).is_empty() {
            return true;
        }
        if !open_four_moves(g, atk, self.win).is_empty() && !g.has_immediate_win(def, self.win) {
            return true;
        }
        for (r, c) in threat_moves(g, atk, self.win) {
            if self.move_forces(g, atk, def, r, c, depth) {
                return true;
            }
            if self.aborted {
                return false;
            }
        }
        false
    }

    /// 进攻方在 `(r,c)` 落子后是否强制胜（含立即五 / 双四 / 冲四递归 / 活三 AND 节点）。
    fn move_forces(&mut self, g: &mut Grid, atk: u8, def: u8, r: i32, c: i32, depth: i32) -> bool {
        if g.would_win(r, c, atk, self.win) {
            return true;
        }
        g.place(r, c, atk);
        let result = self.after_attack(g, atk, def, depth);
        g.unplace(r, c, atk);
        result
    }

    /// 进攻方刚落完一手（已在盘上）后的分类与递归。
    fn after_attack(&mut self, g: &mut Grid, atk: u8, def: u8, depth: i32) -> bool {
        let wps = g.win_points(atk, self.win);
        if wps.len() >= 2 {
            // 活四 / 双四：对手挡不全（除非对手当下能先成五）。
            return !g.has_immediate_win(def, self.win);
        }
        if wps.len() == 1 {
            // 单冲四：对手能先成五则反杀；否则被迫挡唯一成五点。
            if g.has_immediate_win(def, self.win) {
                return false;
            }
            let (br, bc) = wps[0];
            g.place(br, bc, def);
            let res = self.attack(g, atk, def, depth - 1);
            g.unplace(br, bc, def);
            return res;
        }
        // 0 个成五点：是否为活三（存在成活四落点）？
        let ofm = open_four_moves(g, atk, self.win);
        if ofm.is_empty() || g.has_immediate_win(def, self.win) {
            return false;
        }
        // 防守应手的完备超集：成活四落点 ∪ 防守冲四 / 成五点。
        let mut replies = ofm;
        for ((dr, dc), _) in g.four_moves(def, self.win) {
            if !replies.contains(&(dr, dc)) {
                replies.push((dr, dc));
            }
        }
        for (dr, dc) in g.win_points(def, self.win) {
            if !replies.contains(&(dr, dc)) {
                replies.push((dr, dc));
            }
        }
        // 进攻方须对**所有**应手仍能强制胜（应手集合均为空点）。
        for (dr, dc) in replies {
            g.place(dr, dc, def);
            let sub = self.attack(g, atk, def, depth - 1);
            g.unplace(dr, dc, def);
            if self.aborted {
                return false;
            }
            if !sub {
                return false;
            }
        }
        true
    }
}

/// 生成 `atk` 的「威胁手」并按强度降序：立即成五 > 冲四（按成五点数） > 活三（按成活四落点数）。
/// 非威胁手不会强制对手回应，直接剔除——这既大幅剪枝，又把最强手排到最前。
fn threat_moves(g: &mut Grid, atk: u8, win: Win) -> Vec<(i32, i32)> {
    let mut scored: Vec<(i32, (i32, i32))> = Vec::new();
    for (r, c) in g.neighborhood_of(atk, 2) {
        if g.would_win(r, c, atk, win) {
            scored.push((1000, (r, c)));
            continue;
        }
        g.place(r, c, atk);
        let wp = g.win_points(atk, win).len();
        let ofm = if wp == 0 {
            open_four_moves(g, atk, win).len()
        } else {
            0
        };
        g.unplace(r, c, atk);
        if wp >= 1 {
            scored.push((100 + wp as i32, (r, c)));
        } else if ofm >= 1 {
            scored.push((10 + ofm as i32, (r, c)));
        }
    }
    scored.sort_by_key(|&(s, _)| std::cmp::Reverse(s));
    scored.into_iter().map(|(_, p)| p).collect()
}

/// `color` 的「成活四落点」：落子后形成 ≥2 个成五点（活四 / 双四）的空点。
fn open_four_moves(g: &mut Grid, color: u8, win: Win) -> Vec<(i32, i32)> {
    g.four_moves(color, win)
        .into_iter()
        .filter(|&(_, wins)| wins >= 2)
        .map(|(p, _)| p)
        .collect()
}

/// `atk` 是否存在 VCT 强制胜（防守过滤用：判断某候选是否仍让对手有威胁序列杀）。受 `node_cap`
/// / `deadline` 约束；超时 / 超结点即视为「未找到」（保守：宁可漏判威胁也不误杀安全着）。
#[must_use]
pub fn has_vct(
    g: &mut Grid,
    atk: u8,
    win: Win,
    stop: &StopFlag,
    deadline: Instant,
    node_cap: u64,
) -> bool {
    vct_win_move(g, atk, win, stop, deadline, node_cap).is_some()
}

/// 找 `atk` 的一步 VCT 制胜着；无则 `None`。受 `node_cap` / `deadline` 约束。
#[must_use]
pub fn vct_win_move(
    g: &mut Grid,
    atk: u8,
    win: Win,
    stop: &StopFlag,
    deadline: Instant,
    node_cap: u64,
) -> Option<(i32, i32)> {
    let def = if atk == BLACK { WHITE } else { BLACK };
    let mut vct = Vct {
        win,
        stop,
        deadline,
        nodes: 0,
        node_cap,
        aborted: false,
    };
    for (r, c) in threat_moves(g, atk, win) {
        if vct.move_forces(g, atk, def, r, c, VCT_MAX_DEPTH) {
            return Some((r, c));
        }
        if vct.aborted {
            return None;
        }
    }
    None
}
