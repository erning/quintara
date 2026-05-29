//! 落子坐标的字符串编解码：`Position ↔ "X,Y"`（Gomocup 协议坐标）。
//!
//! `X` = 列、`Y` = 行，**均从 0 开始**，与 Gomocup / piskvork AI 协议一致。例：
//! `Position { row: 7, col: 7 } ↔ "7,7"`。wire 协议、棋谱、显示统一用此编码。
//!
//! 注：Gomocup `.psq` 棋谱里坐标是 **1 基**，导出时由 `record` 侧 +1，本模块只负责
//! 内部统一的 0 基 `X,Y`。

use crate::Position;

/// 把坐标编码为 `"X,Y"`（0 基），如 `"7,7"`。
#[must_use]
pub fn encode(pos: Position) -> String {
    format!("{},{}", pos.col, pos.row)
}

/// 解析 `"X,Y"`（0 基）为 [`Position`]；格式非法返回 `None`。
///
/// 不针对具体棋盘尺寸做越界校验——那由调用方相对 [`crate::Board`] 完成。
#[must_use]
pub fn decode(text: &str) -> Option<Position> {
    let (x, y) = text.split_once(',')?;
    let col: u8 = x.trim().parse().ok()?;
    let row: u8 = y.trim().parse().ok()?;
    Some(Position::new(row, col))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_examples() {
        for (pos, text) in [
            (Position::new(0, 0), "0,0"),
            (Position::new(7, 7), "7,7"),
            (Position::new(0, 14), "14,0"),
            (Position::new(19, 0), "0,19"),
        ] {
            assert_eq!(encode(pos), text);
            assert_eq!(decode(text), Some(pos));
        }
    }

    #[test]
    fn decode_rejects_garbage() {
        for bad in ["", "7", "7,", ",7", "a,1", "7,7,7", "7 7"] {
            assert_eq!(decode(bad), None, "should reject {bad:?}");
        }
    }
}
