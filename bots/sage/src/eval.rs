//! 棋型评估（朴素版）：把过某点的一条轴编成 9 格窗口串，匹配棋型给分，4 轴累加。
//!
//! 串里 `C`=本色（含假想落子的中心）、`X`=对手或界外（堵）、`.`=空。匹配从高到低取最高档；
//! 跨 4 轴**累加**——一手成多个威胁（双三、四三）自然得高分。
//!
//! v1 用「窗口串 + 子串匹配」,直白可读;窗口编码查表、bitboard、增量更新等高效结构留待
//! 搜索版（见 roadmap）。

use quintara_model::{Board, Cell, Color, Position};
use quintara_rules::{is_win_for, RuleSet};

/// 四条轴向（各取一个方向，扫描时窗口已含正反两侧）。
const DIRS: [(i32, i32); 4] = [(0, 1), (1, 0), (1, 1), (1, -1)];

/// 把 `pos`（假想为 `color`）沿 `dir` 轴 ±4 的 9 格编成窗口串。
fn window(board: &Board, pos: Position, color: Color, dir: (i32, i32)) -> [u8; 9] {
    let (height, width) = (i32::from(board.height()), i32::from(board.width()));
    let (row0, col0) = (i32::from(pos.row), i32::from(pos.col));
    let mut win = [b'X'; 9];
    for (slot, k) in (-4i32..=4).enumerate() {
        if k == 0 {
            win[slot] = b'C'; // 假想落子
            continue;
        }
        let (row, col) = (row0 + dir.0 * k, col0 + dir.1 * k);
        if row < 0 || col < 0 || row >= height || col >= width {
            continue; // 界外 = 堵（保留 'X'）
        }
        let (Ok(row), Ok(col)) = (u8::try_from(row), u8::try_from(col)) else {
            continue;
        };
        win[slot] = match board.stone_at(Position::new(row, col)) {
            Some(stone) if stone == color => b'C',
            Some(_) => b'X',
            None => b'.',
        };
    }
    win
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// 单轴窗口 → 棋型分（取匹配到的最高档）。
fn axis_score(win: &[u8; 9]) -> i64 {
    if contains(win, b"CCCCC") {
        return 100_000; // 五
    }
    if contains(win, b".CCCC.") {
        return 15_000; // 活四
    }
    // 冲四：填一个空即成五。
    for pat in [b"CCCC." as &[u8], b".CCCC", b"CC.CC", b"CCC.C", b"C.CCC"] {
        if contains(win, pat) {
            return 6_000;
        }
    }
    // 活三：一步成活四。
    for pat in [b".CCC." as &[u8], b".CC.C.", b".C.CC."] {
        if contains(win, pat) {
            return 3_000;
        }
    }
    if contains(win, b"CCC") {
        return 500; // 眠三（被堵）
    }
    for pat in [b".CC." as &[u8], b".C.C."] {
        if contains(win, pat) {
            return 200; // 活二
        }
    }
    if contains(win, b"CC") {
        return 50;
    }
    0
}

/// `color` 假想落子 `pos` 的棋型总分（4 轴累加）。
pub(crate) fn shape_score(board: &Board, pos: Position, color: Color) -> i64 {
    DIRS.iter()
        .map(|&dir| axis_score(&window(board, pos, color, dir)))
        .sum()
}

/// `color` 落子 `pos` 是否立即获胜（按规则正确判定）。
pub(crate) fn wins_at(board: &Board, pos: Position, color: Color, rule_set: RuleSet) -> bool {
    let mut board = board.clone();
    board.set(pos, Cell::Stone(color));
    is_win_for(&board, pos, rule_set, color)
}

/// `pos`（空点）切比雪夫距离 ≤2 内是否有任意一方棋子。
pub(crate) fn near_stone(board: &Board, pos: Position) -> bool {
    let (height, width) = (i32::from(board.height()), i32::from(board.width()));
    let (row0, col0) = (i32::from(pos.row), i32::from(pos.col));
    for d_row in -2..=2 {
        for d_col in -2..=2 {
            if d_row == 0 && d_col == 0 {
                continue;
            }
            let (row, col) = (row0 + d_row, col0 + d_col);
            if row < 0 || col < 0 || row >= height || col >= width {
                continue;
            }
            let (Ok(row), Ok(col)) = (u8::try_from(row), u8::try_from(col)) else {
                continue;
            };
            if board.stone_at(Position::new(row, col)).is_some() {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 在 `cells` 处摆上 `color` 后，评估在 `pos` 假想落子的棋型分。
    fn score_after(cells: &[(u8, u8)], pos: (u8, u8), color: Color) -> i64 {
        let mut board = Board::square(15);
        for &(r, c) in cells {
            board.set(Position::new(r, c), Cell::Stone(color));
        }
        shape_score(&board, Position::new(pos.0, pos.1), color)
    }

    #[test]
    fn shape_tiers_are_ordered() {
        let two = score_after(&[(7, 7)], (7, 8), Color::Black); // 活二
        let open_three = score_after(&[(7, 7), (7, 8)], (7, 9), Color::Black); // .CCC.
        let open_four = score_after(&[(7, 7), (7, 8), (7, 9)], (7, 10), Color::Black); // .CCCC.
        assert!(open_four > open_three, "{open_four} !> {open_three}");
        assert!(open_three > two, "{open_three} !> {two}");
        assert!(two > 0);
    }

    #[test]
    fn fork_outscores_single_threat() {
        // 在 (7,7) 落子同时成两条活三（横 + 竖）→ 应高于单条活三。
        let fork = score_after(&[(7, 5), (7, 6), (5, 7), (6, 7)], (7, 7), Color::Black);
        let single = score_after(&[(7, 5), (7, 6)], (7, 7), Color::Black);
        assert!(fork > single, "fork {fork} !> single {single}");
    }
}
