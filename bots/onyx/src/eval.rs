//! 静态评估与着法排序。
//!
//! 叶子评估直接读 [`Grid`] 增量维护的「五窗计数」分（O(1)）——子力进攻潜力的平滑度量；真正的
//! 战术交给 VCF / 威胁搜索。着法排序则用落点周边的局部连子价值，便宜且方向大致正确。

use crate::grid::{Grid, Win, DIRS, EMPTY};

/// 从 `side` 视角的静态分（O(1) 读增量分）。
#[inline]
#[must_use]
pub fn evaluate(grid: &Grid, side: u8, _win: Win) -> i32 {
    grid.eval_for(side)
}

/// 着法排序键：落点对 `me`（进攻）与 `opp`（防守）的局部连子价值之和，进攻略加权。
#[must_use]
pub fn order_key(grid: &Grid, r: i32, c: i32, me: u8, opp: u8) -> i32 {
    line_value(grid, r, c, me) * 2 + line_value(grid, r, c, opp)
}

/// 把 `color` 落在 `(r,c)` 后，四方向上连子形状的近似价值（仅看连续段 + 两端是否开放）。
fn line_value(grid: &Grid, r: i32, c: i32, color: u8) -> i32 {
    let mut total = 0;
    for (dr, dc) in DIRS {
        let mut len = 1;
        let mut k = 1;
        while grid.code(r + dr * k, c + dc * k) == color {
            len += 1;
            k += 1;
        }
        let open_ahead = grid.code(r + dr * k, c + dc * k) == EMPTY;
        let mut k = 1;
        while grid.code(r - dr * k, c - dc * k) == color {
            len += 1;
            k += 1;
        }
        let open_behind = grid.code(r - dr * k, c - dc * k) == EMPTY;
        let opens = i32::from(open_ahead) + i32::from(open_behind);
        total += match (len, opens) {
            (l, _) if l >= 5 => 100_000,
            (4, 2) => 50_000,
            (4, 1) => 1_200,
            (3, 2) => 1_000,
            (3, 1) => 120,
            (2, 2) => 100,
            (2, 1) => 12,
            (1, 2) => 10,
            _ => 1,
        };
    }
    total
}
