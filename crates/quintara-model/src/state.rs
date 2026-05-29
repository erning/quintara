use crate::{Board, Color, Move};

/// 权威局面。合法着法与胜负均为 `(GameState, RuleSet)` 的纯函数（在 `quintara-rules`
/// 中计算），此处不缓存任何派生簿记字段。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameState {
    pub board: Board,
    pub side_to_move: Color,
    pub move_history: Vec<Move>,
}

impl GameState {
    /// 构造一个空着法历史的局面。
    #[must_use]
    pub fn new(board: Board, side_to_move: Color) -> Self {
        Self {
            board,
            side_to_move,
            move_history: Vec::new(),
        }
    }
}
