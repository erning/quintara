/// 棋盘上的一个交叉点坐标，行列均为 0 基。
///
/// `Position` 本身不绑定棋盘尺寸；越界校验相对具体 [`crate::Board`] 完成
/// （[`crate::Board::in_bounds`]）。`row` / `col` 取值范围由所用规则集的棋盘
/// 尺寸决定（连珠 15、自由式 19）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
    pub row: u8,
    pub col: u8,
}

impl Position {
    #[must_use]
    pub fn new(row: u8, col: u8) -> Self {
        Self { row, col }
    }
}
