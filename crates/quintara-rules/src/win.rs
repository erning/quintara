//! 连子扫描与胜负判定。
//!
//! 所有方向扫描用 `i32` 做带符号坐标运算，回写棋盘前用 `u8::try_from` 校验，避免
//! `as` 截断。

use quintara_model::{Board, Cell, Color, Position};

use crate::ruleset::{RuleSet, WinRule};

/// 四条轴向（每条只取一个方向，扫描时正反两向都走）。
pub(crate) const DIRS: [(i32, i32); 4] = [(0, 1), (1, 0), (1, 1), (1, -1)];

/// 返回把 `pos` 设为 `color` 后的棋盘克隆（用于假想落子的判定）。
pub(crate) fn with_stone(board: &Board, pos: Position, color: Color) -> Board {
    let mut next = board.clone();
    next.set(pos, Cell::Stone(color));
    next
}

/// 经过 `pos`、沿 `dir` 轴的最长同色连子长度（含 `pos` 自身）。
///
/// 前提：`board` 在 `pos` 处已是 `color`（即已真实落子）。
pub(crate) fn dir_run(board: &Board, pos: Position, color: Color, dir: (i32, i32)) -> u32 {
    let width = i32::from(board.width());
    let height = i32::from(board.height());
    let row0 = i32::from(pos.row);
    let col0 = i32::from(pos.col);
    let mut count = 1u32;
    for sign in [1i32, -1i32] {
        let dr = dir.0 * sign;
        let dc = dir.1 * sign;
        let mut step = 1i32;
        loop {
            let row = row0 + dr * step;
            let col = col0 + dc * step;
            if row < 0 || col < 0 || row >= height || col >= width {
                break;
            }
            let (Ok(r), Ok(c)) = (u8::try_from(row), u8::try_from(col)) else {
                break;
            };
            if board.stone_at(Position::new(r, c)) == Some(color) {
                count += 1;
                step += 1;
            } else {
                break;
            }
        }
    }
    count
}

/// 每条轴向经过 `pos` 的连子长度（前提：`pos` 已是 `color`）。
pub(crate) fn dir_runs(board: &Board, pos: Position, color: Color) -> [u32; 4] {
    DIRS.map(|dir| dir_run(board, pos, color, dir))
}

/// 经过 `pos`、沿 `dir` 轴的最长同色连子长度，以及两端紧邻格是否被对方堵死
/// （棋盘外不算堵）。前提：`pos` 已是 `color`。供 caro 判定使用。
fn dir_run_ends(board: &Board, pos: Position, color: Color, dir: (i32, i32)) -> (u32, bool, bool) {
    let width = i32::from(board.width());
    let height = i32::from(board.height());
    let row0 = i32::from(pos.row);
    let col0 = i32::from(pos.col);
    let mut count = 1u32;
    let mut blocked = [false, false];
    for (end, sign) in [1i32, -1i32].into_iter().enumerate() {
        let dr = dir.0 * sign;
        let dc = dir.1 * sign;
        let mut step = 1i32;
        loop {
            let row = row0 + dr * step;
            let col = col0 + dc * step;
            if row < 0 || col < 0 || row >= height || col >= width {
                break; // 棋盘外：不算堵
            }
            let (Ok(r), Ok(c)) = (u8::try_from(row), u8::try_from(col)) else {
                break;
            };
            match board.stone_at(Position::new(r, c)) {
                Some(stone) if stone == color => {
                    count += 1;
                    step += 1;
                }
                Some(_) => {
                    blocked[end] = true; // 对方棋子封堵
                    break;
                }
                None => break, // 空格：开放端
            }
        }
    }
    (count, blocked[0], blocked[1])
}

/// `color` 在 `pos` 落子后是否达成获胜连子（前提：`pos` 已落子），由 `win_rule` 决定。
///
/// - `Overline`：任一轴 ≥ 5（长连算赢）。连珠黑方的长连不会走到这里——它在 `apply` 阶段
///   被 `forbidden_black` 当作禁手拦下（除非同时成五，五连优先）。
/// - `ExactFive`：任一轴**恰好** 5。
/// - `Caro`：任一轴恰好 5，且该五连两端不被对方同时堵死（棋盘外不算堵）。
#[must_use]
pub fn is_win_for(board: &Board, pos: Position, rule_set: RuleSet, color: Color) -> bool {
    match rule_set.win_rule {
        WinRule::Overline => dir_runs(board, pos, color).iter().any(|&run| run >= 5),
        WinRule::ExactFive => dir_runs(board, pos, color).contains(&5),
        WinRule::Caro => DIRS.iter().any(|&dir| {
            let (run, blocked_a, blocked_b) = dir_run_ends(board, pos, color, dir);
            run == 5 && !(blocked_a && blocked_b)
        }),
    }
}

/// 假想 `color` 落子 `pos`（`pos` 当前为空）是否恰好形成五连。
pub(crate) fn makes_exact_five(board: &Board, pos: Position, color: Color) -> bool {
    let placed = with_stone(board, pos, color);
    dir_runs(&placed, pos, color).contains(&5)
}

/// 假想 `color` 落子 `pos`（`pos` 当前为空）后，经过该点的最长同色连子长度。
///
/// 供启发式 bot（如 greedy）评估进攻 / 防守价值。
#[must_use]
pub fn longest_run_if_placed(board: &Board, pos: Position, color: Color) -> u32 {
    let placed = with_stone(board, pos, color);
    dir_runs(&placed, pos, color)
        .iter()
        .copied()
        .max()
        .unwrap_or(0)
}
