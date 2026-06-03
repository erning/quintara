//! 静态评估与着法排序。
//!
//! 叶子评估用「五窗计数」启发：统计每条长度 5 的窗口里**仅含单色 + 空**的子数，按子数加权
//! 累加。它平滑、无负向干扰、足够区分进攻潜力；真正的战术由 VCF / 威胁搜索负责。着法排序则用
//! 落点周边的局部连子价值，便宜且方向大致正确。

use crate::grid::{Grid, Win, BLACK, DIRS, EMPTY, WHITE};

/// 单色窗口按子数加权（下标 = 窗口内该色子数 1..=5）。
const WINDOW_WEIGHT: [i32; 6] = [0, 1, 12, 144, 1728, 200_000];

/// 从 `side` 视角的静态分：`score(side) - score(opp)`。
#[must_use]
pub fn evaluate(grid: &Grid, side: u8, _win: Win) -> i32 {
    let (mut black, mut white) = (0_i32, 0_i32);
    let (w, h) = (grid.width(), grid.height());
    for (dr, dc) in DIRS {
        for r in 0..h {
            for c in 0..w {
                let (er, ec) = (r + dr * 4, c + dc * 4);
                if er < 0 || ec < 0 || er >= h || ec >= w {
                    continue;
                }
                let (mut b, mut wt) = (0, 0);
                for k in 0..5 {
                    match grid.code(r + dr * k, c + dc * k) {
                        BLACK => b += 1,
                        WHITE => wt += 1,
                        _ => {}
                    }
                }
                if wt == 0 && b > 0 {
                    black += WINDOW_WEIGHT[b as usize];
                } else if b == 0 && wt > 0 {
                    white += WINDOW_WEIGHT[wt as usize];
                }
            }
        }
    }
    let (mine, theirs) = if side == BLACK {
        (black, white)
    } else {
        (white, black)
    };
    mine - theirs
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
