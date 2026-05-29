//! 面向**显示 / UI** 的标准棋盘坐标记法：`Position ↔ "H8"`。
//!
//! 横向为字母列（`A` 起，向右递增）、纵向为数字行（**从下往上** `1` 起）。这是连珠 / 五子棋
//! 通行的人类可读记法（如 15×15 天元 = `H8`）。**仅用于展示与人机输入**——内部存储、
//! Gomocup wire 协议、`.psq` 棋谱一律仍用数字 `X,Y`（见 [`crate::coord`]）。
//!
//! 行号依赖棋盘高度（行从下往上数），故编解码都要带 `height`。

use crate::Position;

/// 把坐标格式化为标准记法（如 `"H8"`）。`height` 为棋盘行数（行号自下而上）。
#[must_use]
pub fn format(pos: Position, height: u8) -> String {
    let column = (b'A' + pos.col) as char;
    let row_label = height - pos.row;
    format!("{column}{row_label}")
}

/// 解析标准记法（如 `"H8"` / `"h8"`）为 [`Position`]；格式非法或越界返回 `None`。
/// `height` 用于把自下而上的行号换算回内部自上而下的行索引。
#[must_use]
pub fn parse(text: &str, height: u8) -> Option<Position> {
    let text = text.trim();
    let mut chars = text.chars();
    let letter = chars.next()?.to_ascii_uppercase();
    if !letter.is_ascii_uppercase() {
        return None;
    }
    let col = u8::try_from(u32::from(letter) - u32::from('A')).ok()?;
    let row_label: u8 = chars.as_str().parse().ok()?;
    if row_label == 0 || row_label > height {
        return None;
    }
    let row = height - row_label;
    Some(Position::new(row, col))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn center_of_15x15_is_h8() {
        let center = Position::new(7, 7);
        assert_eq!(format(center, 15), "H8");
        assert_eq!(parse("H8", 15), Some(center));
        assert_eq!(parse("h8", 15), Some(center));
    }

    #[test]
    fn corners_count_rows_from_bottom() {
        // 行索引 0 在最上 → 行号最大；行索引 height-1 在最下 → 行号 1。
        assert_eq!(format(Position::new(0, 0), 15), "A15");
        assert_eq!(format(Position::new(14, 0), 15), "A1");
        assert_eq!(parse("A15", 15), Some(Position::new(0, 0)));
        assert_eq!(parse("A1", 15), Some(Position::new(14, 0)));
    }

    #[test]
    fn round_trip_all_cells() {
        for row in 0..15u8 {
            for col in 0..15u8 {
                let pos = Position::new(row, col);
                assert_eq!(parse(&format(pos, 15), 15), Some(pos));
            }
        }
    }

    #[test]
    fn rejects_garbage_and_out_of_range() {
        for bad in ["", "8", "H", "H0", "H16", "1H", "ZZ"] {
            assert_eq!(parse(bad, 15), None, "should reject {bad:?}");
        }
    }
}
