use crate::Position;

/// 一手着法。五子棋只有「落子」这一种动作——不翻子、不移子、不跳过。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Move {
    Place(Position),
}

impl Move {
    #[must_use]
    pub fn position(self) -> Position {
        match self {
            Move::Place(pos) => pos,
        }
    }
}
