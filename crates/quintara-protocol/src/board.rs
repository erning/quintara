//! `BOARD` 命令里的棋盘格编码：`"X,Y,field"`，`field` ∈ `1`(己方) / `2`(对方) /
//! `3`(获胜连子或连珠禁手点，仅连续局)。

use quintara_model::{coord, Position};

use crate::{parse_coord, ParseError};

/// 棋盘格归属。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    /// 己方棋子（`1`）。
    Own,
    /// 对方棋子（`2`）。
    Opp,
    /// 获胜连子 / 禁手点（`3`，仅连续局）。
    Winning,
}

impl Field {
    #[must_use]
    pub fn code(self) -> u8 {
        match self {
            Field::Own => 1,
            Field::Opp => 2,
            Field::Winning => 3,
        }
    }

    #[must_use]
    pub fn from_code(code: u8) -> Option<Self> {
        match code {
            1 => Some(Field::Own),
            2 => Some(Field::Opp),
            3 => Some(Field::Winning),
            _ => None,
        }
    }
}

/// `BOARD` 数据中的一格。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoardCell {
    pub pos: Position,
    pub field: Field,
}

/// 编码为 `"X,Y,field"`。
#[must_use]
pub fn encode_cell(cell: BoardCell) -> String {
    format!("{},{}", coord::encode(cell.pos), cell.field.code())
}

/// 解析 `"X,Y,field"`。
///
/// # Errors
/// 坐标或 `field` 非法时返回 [`ParseError`]。
pub fn decode_cell(line: &str) -> Result<BoardCell, ParseError> {
    let line = line.trim();
    let mut parts = line.rsplitn(2, ',');
    let field_str = parts
        .next()
        .ok_or_else(|| ParseError::Malformed(line.to_string()))?;
    let coord_str = parts
        .next()
        .ok_or_else(|| ParseError::Malformed(line.to_string()))?;
    let code: u8 = field_str
        .trim()
        .parse()
        .map_err(|_| ParseError::BadField(field_str.to_string()))?;
    let field =
        Field::from_code(code).ok_or_else(|| ParseError::BadField(field_str.to_string()))?;
    let pos = parse_coord(coord_str)?;
    Ok(BoardCell { pos, field })
}
