use crate::{Color, Position};

/// 单个交叉点的状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell {
    Empty,
    Stone(Color),
}

/// `width × height` 的交叉点网格。棋子落下后永不移动、永不翻面、永不被提走。
///
/// 内核以宽高分离表示——天然支持矩形盘（对应 Gomocup `RECTSTART`）；常用且默认是正方形
/// （`Board::square`）。坐标 `Position{row, col}`：`row` 取 `0..height`，`col` 取 `0..width`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Board {
    width: u8,
    height: u8,
    cells: Vec<Cell>,
}

impl Board {
    /// 构造一个 `width × height` 的空盘（矩形）。
    #[must_use]
    pub fn rect(width: u8, height: u8) -> Self {
        let cells = vec![Cell::Empty; usize::from(width) * usize::from(height)];
        Self {
            width,
            height,
            cells,
        }
    }

    /// 构造一个 `size × size` 的空盘（正方形，默认主路径）。
    #[must_use]
    pub fn square(size: u8) -> Self {
        Self::rect(size, size)
    }

    #[must_use]
    pub fn width(&self) -> u8 {
        self.width
    }

    #[must_use]
    pub fn height(&self) -> u8 {
        self.height
    }

    /// 正方形盘的边长；矩形盘返回 `None`。
    #[must_use]
    pub fn square_size(&self) -> Option<u8> {
        (self.width == self.height).then_some(self.width)
    }

    #[must_use]
    pub fn in_bounds(&self, pos: Position) -> bool {
        pos.col < self.width && pos.row < self.height
    }

    fn index(&self, pos: Position) -> usize {
        usize::from(pos.row) * usize::from(self.width) + usize::from(pos.col)
    }

    /// 返回该点的状态；越界返回 `None`。
    #[must_use]
    pub fn get(&self, pos: Position) -> Option<Cell> {
        if self.in_bounds(pos) {
            Some(self.cells[self.index(pos)])
        } else {
            None
        }
    }

    /// 设置该点状态。越界则静默忽略——调用方应先用 [`Board::in_bounds`] 校验。
    pub fn set(&mut self, pos: Position, cell: Cell) {
        if self.in_bounds(pos) {
            let i = self.index(pos);
            self.cells[i] = cell;
        }
    }

    #[must_use]
    pub fn is_empty_at(&self, pos: Position) -> bool {
        self.get(pos) == Some(Cell::Empty)
    }

    /// 该点的棋子颜色；空点或越界返回 `None`。
    #[must_use]
    pub fn stone_at(&self, pos: Position) -> Option<Color> {
        match self.get(pos) {
            Some(Cell::Stone(c)) => Some(c),
            _ => None,
        }
    }

    /// 棋盘是否已被填满（无空点）。
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.cells.iter().all(|c| !matches!(c, Cell::Empty))
    }

    /// 所有空交叉点，按行序（先 row 后 col）排列。
    #[must_use]
    pub fn empty_positions(&self) -> Vec<Position> {
        let mut out = Vec::new();
        for row in 0..self.height {
            for col in 0..self.width {
                let pos = Position::new(row, col);
                if self.is_empty_at(pos) {
                    out.push(pos);
                }
            }
        }
        out
    }
}
