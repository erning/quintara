//! 连珠黑方禁手判定：长连（overline）/ 双四（double-four）/ 双三（double-three），
//! 依据 RIF《International Rules of Renju》（见 `docs/rules.md §6`）。
//!
//! 判定在「沿某轴的一维编码」上做模式匹配：`1` = 黑子，`0` = 空，`-1` = 白子或棋盘
//! 外（对黑方而言都是封堵）。每条轴线两端补 `-1` 哨兵。
//!
//! 双三的递归例外（§6.3）用带深度上限的递归实现：一个「三」只有当「能把它变成
//! 活四的那个点」本身不是黑方禁手时才算「真三」。已知边界：极深嵌套禁手可能偏
//! 保守，单元测试覆盖标准用例。

use quintara_model::{Board, Color, Position};

use crate::win::{dir_runs, with_stone, DIRS};

/// 递归深度上限——超出后保守地认为「该点合法」（不再下钻），避免病态局面下的
/// 指数级展开。常规连珠局面递归很浅，此上限不影响标准判定。
const MAX_DEPTH: u32 = 5;

/// 黑方在 `pos`（当前为空）落子是否构成禁手。
///
/// 前提：轮到黑方、`pos` 在界内且为空。**不**在此判断「同时成五」的优先权——
/// 调用方（`apply` / `legal`）须先用 `win::makes_exact_five` 放行成五的着法。
#[must_use]
pub fn is_forbidden(board: &Board, pos: Position) -> bool {
    let placed = with_stone(board, pos, Color::Black);
    classify(&placed, pos, 0)
}

/// `b` 中 `stone` 已是黑子；判断「在此落黑子」是否禁手。
fn classify(b: &Board, stone: Position, depth: u32) -> bool {
    // 五连优先：成五即合法。
    if dir_runs(b, stone, Color::Black).contains(&5) {
        return false;
    }
    // 长连。
    if dir_runs(b, stone, Color::Black).iter().any(|&run| run >= 6) {
        return true;
    }
    // 双四。
    if count_fours(b, stone) >= 2 {
        return true;
    }
    // 双三（含递归例外）。
    if count_real_threes(b, stone, depth) >= 2 {
        return true;
    }
    false
}

/// 经过 `stone` 的「四」的总数（四条轴求和）。
fn count_fours(b: &Board, stone: Position) -> u32 {
    let mut total = 0u32;
    for dir in DIRS {
        let (cells, _pos, idx) = axis_line(b, stone, dir);
        total += count_fours_dir(&cells, idx);
    }
    total
}

/// 经过 `stone` 的「真三」的数量（四条轴各至多算一个）。
fn count_real_threes(b: &Board, stone: Position, depth: u32) -> u32 {
    let mut total = 0u32;
    for dir in DIRS {
        if dir_has_real_three(b, stone, dir, depth) {
            total += 1;
        }
    }
    total
}

/// 沿 `dir` 轴是否存在经过 `stone` 的「真三」。
///
/// 「三」= 再加一子可成活四（且不同时成五）。「真」= 那个使其成活四的点本身不是黑方
/// 禁手（递归例外，§6.3）。
fn dir_has_real_three(b: &Board, stone: Position, dir: (i32, i32), depth: u32) -> bool {
    let (cells, positions, idx) = axis_line(b, stone, dir);
    for j in 0..cells.len() {
        if cells[j] != 0 {
            continue;
        }
        let Some(making_point) = positions[j] else {
            continue;
        };
        let mut trial = cells.clone();
        trial[j] = 1;
        // 填 j 后须形成经过 j 与 stone 的活四，且不是五连。
        if !open_four_contains(&trial, j, idx) {
            continue;
        }
        if run_through(&trial, j) >= 5 {
            continue;
        }
        // 该三成立——检查「成活四的点」是否合法（递归例外）。
        let making_point_legal = depth >= MAX_DEPTH || {
            let next = with_stone(b, making_point, Color::Black);
            !classify(&next, making_point, depth + 1)
        };
        if making_point_legal {
            return true;
        }
    }
    false
}

/// 沿 `dir` 轴、经过 `point` 的一维编码线，附每格对应的棋盘坐标（哨兵为 `None`）。
fn axis_line(
    board: &Board,
    point: Position,
    dir: (i32, i32),
) -> (Vec<i8>, Vec<Option<Position>>, usize) {
    let width = i32::from(board.width());
    let height = i32::from(board.height());
    // 退到该轴最靠 `-dir` 的一端。
    let mut start_row = i32::from(point.row);
    let mut start_col = i32::from(point.col);
    loop {
        let prev_row = start_row - dir.0;
        let prev_col = start_col - dir.1;
        if prev_row < 0 || prev_col < 0 || prev_row >= height || prev_col >= width {
            break;
        }
        start_row = prev_row;
        start_col = prev_col;
    }

    let mut cells = vec![-1i8];
    let mut positions: Vec<Option<Position>> = vec![None];
    let mut idx = 0;
    let (mut row, mut col) = (start_row, start_col);
    while row >= 0 && col >= 0 && row < height && col < width {
        let (Ok(r), Ok(c)) = (u8::try_from(row), u8::try_from(col)) else {
            break;
        };
        let pos = Position::new(r, c);
        let code = match board.stone_at(pos) {
            Some(Color::Black) => 1,
            Some(Color::White) => -1,
            None => 0,
        };
        if pos == point {
            idx = cells.len();
        }
        cells.push(code);
        positions.push(Some(pos));
        row += dir.0;
        col += dir.1;
    }
    cells.push(-1);
    positions.push(None);
    (cells, positions, idx)
}

/// 经过 `idx` 的「四」的数量（同一轴内）。活四（两端可成五）算一个四，不算两个。
fn count_fours_dir(cells: &[i8], idx: usize) -> u32 {
    let mut gaps: Vec<usize> = Vec::new();
    let upper = cells.len().saturating_sub(4);
    for start in 0..upper {
        if idx < start || idx >= start + 5 {
            continue;
        }
        let window = &cells[start..start + 5];
        let ones = window.iter().filter(|&&x| x == 1).count();
        let zeros = window.iter().filter(|&&x| x == 0).count();
        if ones == 4 && zeros == 1 {
            if let Some(offset) = window.iter().position(|&x| x == 0) {
                let gap = start + offset;
                if !gaps.contains(&gap) {
                    gaps.push(gap);
                }
            }
        }
    }
    let mut fours = u32::try_from(gaps.len()).unwrap_or(u32::MAX);
    // 活四产生两个补全点却只是一个四——修正。
    if has_open_four_through(cells, idx) {
        fours = fours.saturating_sub(1);
    }
    fours
}

/// `cells[start..start+6] == [0,1,1,1,1,0]`。
fn is_open_four_at(cells: &[i8], start: usize) -> bool {
    start + 6 <= cells.len() && cells[start..start + 6] == [0i8, 1, 1, 1, 1, 0]
}

/// 是否存在经过 `idx` 的活四窗口。
fn has_open_four_through(cells: &[i8], idx: usize) -> bool {
    let upper = cells.len().saturating_sub(5);
    (0..upper).any(|start| is_open_four_at(cells, start) && (start..start + 6).contains(&idx))
}

/// 是否存在同时经过 `a` 与 `b` 的活四窗口。
fn open_four_contains(cells: &[i8], a: usize, b: usize) -> bool {
    let upper = cells.len().saturating_sub(5);
    (0..upper).any(|start| {
        is_open_four_at(cells, start)
            && (start..start + 6).contains(&a)
            && (start..start + 6).contains(&b)
    })
}

/// 经过 `idx` 的连续 `1` 的长度。
fn run_through(cells: &[i8], idx: usize) -> usize {
    if cells.get(idx) != Some(&1) {
        return 0;
    }
    let mut len = 1usize;
    let mut left = idx;
    while left > 0 && cells[left - 1] == 1 {
        len += 1;
        left -= 1;
    }
    let mut right = idx;
    while right + 1 < cells.len() && cells[right + 1] == 1 {
        len += 1;
        right += 1;
    }
    len
}
